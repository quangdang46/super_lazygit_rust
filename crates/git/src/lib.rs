use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super_lazygit_core::state::GitFlowBranchType;
use super_lazygit_core::{
    BisectState, BranchItem, CommitDivergence, CommitFileItem, CommitHistoryMode, CommitItem,
    CommitStatus, CommitTodoAction, ComparisonTarget, Diagnostics, DiagnosticsSnapshot, DiffHunk,
    DiffLine, DiffLineKind, DiffModel, DiffPresentation, FileStatus, FileStatusKind, GitCommand,
    GitCommandRequest, HeadKind, MergeFastForwardPreference, MergeState, MergeVariant,
    PatchApplicationMode, RebaseKind, RebaseStartMode, RebaseState, ReflogItem, RemoteBranchItem,
    RemoteItem, RemoteSummary, RepoDetail, RepoId, RepoSummary, ResetMode, SelectedHunk, StashItem,
    StashMode, SubmoduleItem, TagItem, Timestamp, WatcherFreshness, WorkingTreeState, WorktreeItem,
};
use thiserror::Error;

mod graph;

use crate::graph::{render_commit_graph, GraphCommit};

const GIT_OPTIONAL_LOCKS_ENV: &str = "GIT_OPTIONAL_LOCKS";
const GIT_OPTIONAL_LOCKS_DISABLED: &str = "0";
const GIT_INDEX_LOCK_MARKER: &str = ".git/index.lock";
const GIT_INDEX_LOCK_RETRY_COUNT: usize = 5;
const GIT_INDEX_LOCK_RETRY_WAIT: Duration = Duration::from_millis(50);
const DEFAULT_MAIN_BRANCH_NAMES: [&str; 2] = ["master", "main"];

#[derive(Debug, Clone, PartialEq, Eq)]
struct GitCommandBuilder {
    args: Vec<OsString>,
}

#[allow(dead_code)]
impl GitCommandBuilder {
    fn new(command: impl Into<OsString>) -> Self {
        Self {
            args: vec![command.into()],
        }
    }

    fn arg<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    fn arg_if<I, S>(self, condition: bool, if_true: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        if condition {
            self.arg(if_true)
        } else {
            self
        }
    }

    fn arg_if_else(
        mut self,
        condition: bool,
        if_true: impl Into<OsString>,
        if_false: impl Into<OsString>,
    ) -> Self {
        self.args.push(if condition {
            if_true.into()
        } else {
            if_false.into()
        });
        self
    }

    fn config(self, value: impl Into<OsString>) -> Self {
        self.prepend_pair("-c", value)
    }

    fn config_if(self, condition: bool, value: impl Into<OsString>) -> Self {
        if condition {
            self.config(value)
        } else {
            self
        }
    }

    fn dir(self, path: impl Into<OsString>) -> Self {
        self.prepend_pair("-C", path)
    }

    fn dir_if(self, condition: bool, path: impl Into<OsString>) -> Self {
        if condition {
            self.dir(path)
        } else {
            self
        }
    }

    fn worktree(self, path: impl Into<OsString>) -> Self {
        self.prepend_pair("--work-tree", path)
    }

    fn worktree_path_if(self, condition: bool, path: impl Into<OsString>) -> Self {
        if condition {
            self.worktree(path)
        } else {
            self
        }
    }

    fn git_dir(self, path: impl Into<OsString>) -> Self {
        self.prepend_pair("--git-dir", path)
    }

    fn git_dir_if(self, condition: bool, path: impl Into<OsString>) -> Self {
        if condition {
            self.git_dir(path)
        } else {
            self
        }
    }

    fn to_argv(&self) -> Vec<OsString> {
        std::iter::once(OsString::from("git"))
            .chain(self.args.iter().cloned())
            .collect()
    }

    fn into_args(self) -> Vec<OsString> {
        self.args
    }

    fn prepend_pair(mut self, flag: &'static str, value: impl Into<OsString>) -> Self {
        self.args.splice(0..0, [OsString::from(flag), value.into()]);
        self
    }
}

impl fmt::Display for GitCommandBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.to_argv()
                .iter()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(" ")
        )
    }
}

pub trait GitBackend: Send + Sync + 'static {
    fn kind(&self) -> GitBackendKind;

    fn scan_workspace(&self, request: WorkspaceScanRequest) -> GitResult<WorkspaceScanResult>;

    fn read_repo_summary(&self, request: RepoSummaryRequest) -> GitResult<RepoSummary>;

    fn read_repo_detail(&self, request: RepoDetailRequest) -> GitResult<RepoDetail>;

    fn read_branch_merge_status(&self, request: BranchMergeStatusRequest) -> GitResult<bool>;

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

    pub fn read_branch_merge_status(
        &mut self,
        request: BranchMergeStatusRequest,
    ) -> GitResult<bool> {
        let operation = GitOperationKind::ReadBranchMergeStatus;
        self.execute_routed(operation, |backend| {
            backend.read_branch_merge_status(request)
        })
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
    ReadBranchMergeStatus,
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
            Self::ReadBranchMergeStatus => "read_branch_merge_status",
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
            Self::ReadBranchMergeStatus => GitBackendCapability::DetailRead,
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
            GitOperationKind::ReadBranchMergeStatus => self.detail_reads,
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
pub struct BranchMergeStatusRequest {
    pub repo_id: RepoId,
    pub branch_name: String,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoPaths {
    worktree_path: PathBuf,
    worktree_git_dir_path: PathBuf,
    repo_path: PathBuf,
    repo_git_dir_path: PathBuf,
    repo_name: String,
    is_bare_repo: bool,
}

impl RepoPaths {
    pub fn resolve(repo_path: &Path) -> GitResult<Self> {
        let resolved_repo_path = canonicalize_existing_path(repo_path);
        match git_stdout(
            &resolved_repo_path,
            [
                "rev-parse",
                "--path-format=absolute",
                "--show-toplevel",
                "--absolute-git-dir",
                "--git-common-dir",
                "--is-bare-repository",
                "--show-superproject-working-tree",
            ],
        ) {
            Ok(output) => parse_repo_paths_output(&resolved_repo_path, &output),
            Err(primary_error) => {
                let fallback_output = git_stdout(
                    &resolved_repo_path,
                    [
                        "rev-parse",
                        "--path-format=absolute",
                        "--absolute-git-dir",
                        "--git-common-dir",
                        "--is-bare-repository",
                        "--show-superproject-working-tree",
                    ],
                )?;
                let fallback_paths =
                    parse_bare_repo_paths_output(&resolved_repo_path, &fallback_output)?;
                if fallback_paths.is_bare_repo() {
                    Ok(fallback_paths)
                } else {
                    Err(primary_error)
                }
            }
        }
    }

    #[must_use]
    pub fn worktree_path(&self) -> &Path {
        &self.worktree_path
    }

    #[must_use]
    pub fn worktree_git_dir_path(&self) -> &Path {
        &self.worktree_git_dir_path
    }

    #[must_use]
    pub fn git_dir(&self) -> &Path {
        self.worktree_git_dir_path()
    }

    #[must_use]
    pub fn repo_path(&self) -> &Path {
        &self.repo_path
    }

    #[must_use]
    pub fn repo_git_dir_path(&self) -> &Path {
        &self.repo_git_dir_path
    }

    #[must_use]
    pub fn repo_name(&self) -> &str {
        &self.repo_name
    }

    #[must_use]
    pub fn is_bare_repo(&self) -> bool {
        self.is_bare_repo
    }
}

fn parse_repo_paths_output(resolved_repo_path: &Path, output: &str) -> GitResult<RepoPaths> {
    let results = output.lines().collect::<Vec<_>>();
    if results.len() < 4 {
        return Err(repo_paths_parse_error(output));
    }

    let worktree_path = canonicalize_existing_path(Path::new(results[0]));
    let worktree_git_dir_path = canonicalize_existing_path(Path::new(results[1]));
    let repo_git_dir_path = canonicalize_existing_path(Path::new(results[2]));
    let is_bare_repo = results[3] == "true";
    let is_submodule = results.get(4).is_some_and(|value| !value.is_empty());
    let repo_path = if is_submodule {
        worktree_path.clone()
    } else {
        canonicalize_existing_path(repo_git_dir_path.parent().unwrap_or(resolved_repo_path))
    };

    Ok(RepoPaths {
        repo_name: repo_name_from_path(&repo_path),
        worktree_path,
        worktree_git_dir_path,
        repo_path,
        repo_git_dir_path,
        is_bare_repo,
    })
}

fn parse_bare_repo_paths_output(resolved_repo_path: &Path, output: &str) -> GitResult<RepoPaths> {
    let results = output.lines().collect::<Vec<_>>();
    if results.len() < 3 {
        return Err(repo_paths_parse_error(output));
    }

    let worktree_git_dir_path = canonicalize_existing_path(Path::new(results[0]));
    let repo_git_dir_path = canonicalize_existing_path(Path::new(results[1]));
    let is_bare_repo = results[2] == "true";
    let repo_path =
        canonicalize_existing_path(repo_git_dir_path.parent().unwrap_or(resolved_repo_path));

    Ok(RepoPaths {
        repo_name: repo_name_from_path(&repo_path),
        worktree_path: resolved_repo_path.to_path_buf(),
        worktree_git_dir_path,
        repo_path,
        repo_git_dir_path,
        is_bare_repo,
    })
}

fn repo_name_from_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string()
}

fn repo_paths_parse_error(output: &str) -> GitError {
    GitError::OperationFailed {
        message: format!("unexpected rev-parse repo paths output: {output:?}"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomCommands {
    repo_path: PathBuf,
}

impl CustomCommands {
    #[must_use]
    pub fn new(repo_path: impl Into<PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
        }
    }

    pub fn run_with_output(&self, cmd_str: &str) -> GitResult<String> {
        let argv = parse_custom_command_args(cmd_str)?;
        let (program, args) = argv.split_first().expect("custom command has argv");
        let output = Command::new(program)
            .args(args)
            .current_dir(&self.repo_path)
            .output()
            .map_err(io_error)?;
        if !output.status.success() {
            return Err(process_failure(program, output));
        }
        stdout_raw_string(output)
    }

    pub fn template_function_run_command(&self, cmd_str: &str) -> GitResult<String> {
        let output = self.run_with_output(cmd_str)?;
        let output = output.trim_end_matches(['\r', '\n']).to_string();
        if output.contains("\r\n") {
            return Err(GitError::OperationFailed {
                message: format!("command output contains newlines: {output}"),
            });
        }
        Ok(output)
    }
}

fn parse_custom_command_args(command: &str) -> GitResult<Vec<String>> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum QuoteMode {
        Single,
        Double,
    }

    let mut args = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();
    let mut quote_mode = None;
    let mut token_started = false;

    while let Some(ch) = chars.next() {
        match quote_mode {
            Some(QuoteMode::Single) => {
                if ch == '\'' {
                    quote_mode = None;
                } else {
                    current.push(ch);
                }
                token_started = true;
            }
            Some(QuoteMode::Double) => {
                if ch == '"' {
                    quote_mode = None;
                } else if ch == '\\' {
                    let escaped = chars.next().ok_or_else(|| GitError::OperationFailed {
                        message: "unterminated escape in custom command".to_string(),
                    })?;
                    current.push(escaped);
                } else {
                    current.push(ch);
                }
                token_started = true;
            }
            None => match ch {
                '\'' => {
                    quote_mode = Some(QuoteMode::Single);
                    token_started = true;
                }
                '"' => {
                    quote_mode = Some(QuoteMode::Double);
                    token_started = true;
                }
                '\\' => {
                    let escaped = chars.next().ok_or_else(|| GitError::OperationFailed {
                        message: "unterminated escape in custom command".to_string(),
                    })?;
                    current.push(escaped);
                    token_started = true;
                }
                ch if ch.is_whitespace() => {
                    if token_started {
                        args.push(std::mem::take(&mut current));
                        token_started = false;
                    }
                }
                _ => {
                    current.push(ch);
                    token_started = true;
                }
            },
        }
    }

    if quote_mode.is_some() {
        return Err(GitError::OperationFailed {
            message: "unterminated quote in custom command".to_string(),
        });
    }

    if token_started {
        args.push(current);
    }

    if args.is_empty() {
        return Err(GitError::OperationFailed {
            message: "custom command is empty".to_string(),
        });
    }

    Ok(args)
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

    fn read_branch_merge_status(&self, request: BranchMergeStatusRequest) -> GitResult<bool> {
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

        let parsed = read_status_snapshot(
            &repo_path,
            super_lazygit_core::DEFAULT_RENAME_SIMILARITY_THRESHOLD,
        )?;
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
        let status = read_status_snapshot(&repo_path, request.rename_similarity_threshold)?;
        let selected_path = request.selected_path.clone();
        let diff = read_diff_model(
            &repo_path,
            None,
            None,
            selected_path.clone().or(status.first_path.clone()),
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
        let branches = read_branches(&repo_path);
        let remote_branches = read_remote_branches(&repo_path);
        let fast_forward_merge_targets =
            read_fast_forward_merge_targets(&repo_path, &branches, &remote_branches);
        let remotes = read_remotes(&repo_path, &remote_branches);
        let submodules = read_submodules(&repo_path);
        let working_tree_state = read_working_tree_state(&repo_path);
        Ok(RepoDetail {
            file_tree: status.file_tree,
            diff,
            branches,
            remotes,
            remote_branches,
            tags: read_tags(&repo_path),
            commits: commit_history.commits,
            commit_graph_lines: commit_history.graph_lines,
            bisect_state: read_bisect_state(&repo_path),
            rebase_state: read_rebase_state(&repo_path),
            stashes: read_stashes(&repo_path, selected_path.as_deref()),
            reflog_items: read_reflog(&repo_path),
            worktrees: read_worktrees(&repo_path),
            submodules,
            working_tree_state,
            commit_input: String::new(),
            merge_state: read_merge_state(working_tree_state),
            merge_fast_forward_preference: read_merge_fast_forward_preference(&repo_path),
            fast_forward_merge_targets,
        })
    }

    fn read_branch_merge_status(&self, request: BranchMergeStatusRequest) -> GitResult<bool> {
        let repo_path = repo_path(&request.repo_id)?;
        is_branch_merged(&repo_path, &request.branch_name)
    }

    fn read_diff(&self, request: DiffRequest) -> GitResult<DiffModel> {
        let repo_path = repo_path(&request.repo_id)?;
        let selected_path = match request.selected_path {
            Some(path) => Some(path),
            None => {
                read_status_snapshot(&repo_path, request.rename_similarity_threshold)?.first_path
            }
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
            GitCommand::StartBisect { commit, term } => {
                git(&repo_path, ["bisect", "start"])?;
                git(&repo_path, ["bisect", term.as_str(), commit])?;
                let short = git_stdout(&repo_path, ["rev-parse", "--short", commit.as_str()])
                    .unwrap_or_else(|_| commit.clone());
                let subject = git_stdout(&repo_path, ["show", "-s", "--format=%s", commit])
                    .unwrap_or_else(|_| commit.clone());
                format!("Started bisect by marking {short} {subject} as {term}")
            }
            GitCommand::MarkBisect { commit, term } => {
                git(&repo_path, ["bisect", term.as_str(), commit])?;
                let short = git_stdout(&repo_path, ["rev-parse", "--short", commit.as_str()])
                    .unwrap_or_else(|_| commit.clone());
                let subject = git_stdout(&repo_path, ["show", "-s", "--format=%s", commit])
                    .unwrap_or_else(|_| commit.clone());
                format!("Marked {short} {subject} as {term} for bisect")
            }
            GitCommand::SkipBisect { commit } => {
                git(&repo_path, ["bisect", "skip", commit])?;
                let short = git_stdout(&repo_path, ["rev-parse", "--short", commit.as_str()])
                    .unwrap_or_else(|_| commit.clone());
                let subject = git_stdout(&repo_path, ["show", "-s", "--format=%s", commit])
                    .unwrap_or_else(|_| commit.clone());
                format!("Skipped {short} {subject} during bisect")
            }
            GitCommand::ResetBisect => {
                git(&repo_path, ["bisect", "reset"])?;
                "Reset active bisect".to_string()
            }
            GitCommand::CreateFixupCommit { commit } => {
                git(&repo_path, ["commit", "--fixup", commit])?;
                let short = git_stdout(&repo_path, ["rev-parse", "--short", commit.as_str()])
                    .unwrap_or_else(|_| commit.clone());
                let subject = git_stdout(&repo_path, ["show", "-s", "--format=%s", commit])
                    .unwrap_or_else(|_| commit.clone());
                format!("Created fixup commit for {short} {subject}")
            }
            GitCommand::CreateAmendCommit {
                original_subject,
                message,
                include_file_changes,
            } => {
                let amend_subject = format!("amend! {original_subject}");
                git_builder(
                    &repo_path,
                    GitCommandBuilder::new("commit")
                        .arg(["-m", amend_subject.as_str(), "-m", message.as_str()])
                        .arg_if(!*include_file_changes, ["--only", "--allow-empty"]),
                )?;
                if *include_file_changes {
                    format!("Created amend! commit with changes for {original_subject}")
                } else {
                    format!("Created amend! commit without changes for {original_subject}")
                }
            }
            GitCommand::AmendCommitAttributes {
                commit,
                reset_author,
                co_author,
            } => {
                amend_commit_attributes(&repo_path, commit, *reset_author, co_author.as_deref())?;
                let short = git_stdout(&repo_path, ["rev-parse", "--short", commit.as_str()])
                    .unwrap_or_else(|_| commit.clone());
                let subject = git_stdout(&repo_path, ["show", "-s", "--format=%s", commit])
                    .unwrap_or_else(|_| commit.clone());
                if *reset_author {
                    format!("Reset author for {short} {subject}")
                } else {
                    format!("Set co-author for {short} {subject}")
                }
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
                    RebaseStartMode::FixupWithMessage => {
                        format!("Set fixup message from {short} {subject}")
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
                git_builder_with_env(
                    &repo_path,
                    GitCommandBuilder::new("rebase").arg(["--continue"]),
                    &[("GIT_EDITOR", OsStr::new(":"))],
                )?;
                "Continued rebase".to_string()
            }
            GitCommand::AbortRebase => {
                git(&repo_path, ["rebase", "--abort"])?;
                "Aborted rebase".to_string()
            }
            GitCommand::SkipRebase => {
                git_builder_with_env(
                    &repo_path,
                    GitCommandBuilder::new("rebase").arg(["--skip"]),
                    &[("GIT_EDITOR", OsStr::new(":"))],
                )?;
                "Skipped current rebase step".to_string()
            }
            GitCommand::CreateBranch { branch_name } => {
                git(&repo_path, ["checkout", "-b", branch_name.as_str()])?;
                format!("Created and checked out {branch_name}")
            }
            GitCommand::StartGitFlow { branch_type, name } => {
                git(&repo_path, git_flow_start_args(*branch_type, name))?;
                format!("Started git-flow {} {name}", branch_type.command_name())
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
                track,
            } => {
                git(
                    &repo_path,
                    create_branch_from_ref_args(branch_name, start_point, *track),
                )?;
                if *track {
                    format!("Created {branch_name} tracking {start_point}")
                } else {
                    format!("Created {branch_name} from {start_point} without tracking")
                }
            }
            GitCommand::FinishGitFlow { branch_name } => {
                let (branch_type, suffix) = resolve_git_flow_finish_parts(&repo_path, branch_name)?;
                git(&repo_path, git_flow_finish_args(&branch_type, &suffix))?;
                format!("Finished git-flow {branch_type} {suffix}")
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
            GitCommand::ForceCheckoutRef { target_ref } => {
                git(&repo_path, ["checkout", "-f", target_ref.as_str()])?;
                format!("Force-checked out {target_ref}")
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
            GitCommand::DeleteBranch { branch_name, force } => {
                git(&repo_path, delete_branch_args(branch_name, *force))?;
                if *force {
                    format!("Force-deleted {branch_name}")
                } else {
                    format!("Deleted {branch_name}")
                }
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
                    StashMode::Staged => stash_staged_changes(&repo_path, message.as_deref())?,
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
                git_builder(
                    &repo_path,
                    GitCommandBuilder::new("worktree")
                        .arg(["add"])
                        .arg([path.as_os_str(), OsStr::new(branch_ref)]),
                )?;
                format!("Created worktree {} from {branch_ref}", path.display())
            }
            GitCommand::RemoveWorktree { path } => {
                git_builder(
                    &repo_path,
                    GitCommandBuilder::new("worktree")
                        .arg(["remove"])
                        .arg([path.as_os_str()]),
                )?;
                format!("Removed worktree {}", path.display())
            }
            GitCommand::AddSubmodule { path, url } => {
                let path_value = path.to_string_lossy().into_owned();
                git_builder(
                    &repo_path,
                    GitCommandBuilder::new("submodule")
                        .config("protocol.file.allow=always")
                        .arg(["add", url.as_str(), path_value.as_str()]),
                )?;
                format!("Added submodule {} from {url}", path.display())
            }
            GitCommand::EditSubmoduleUrl { name, path, url } => {
                edit_submodule_url(&repo_path, name, path, url)?;
                format!("Updated submodule {} URL", path.display())
            }
            GitCommand::InitSubmodule { path } => {
                let path_value = path.to_string_lossy().into_owned();
                git_builder(
                    &repo_path,
                    GitCommandBuilder::new("submodule")
                        .config("protocol.file.allow=always")
                        .arg(["update", "--init", "--", path_value.as_str()]),
                )?;
                format!("Initialized submodule {}", path.display())
            }
            GitCommand::UpdateSubmodule { path } => {
                let path_value = path.to_string_lossy().into_owned();
                git_builder(
                    &repo_path,
                    GitCommandBuilder::new("submodule")
                        .config("protocol.file.allow=always")
                        .arg(["update", "--remote", "--", path_value.as_str()]),
                )?;
                format!("Updated submodule {}", path.display())
            }
            GitCommand::InitAllSubmodules => {
                git(&repo_path, ["submodule", "init"])?;
                "Initialized all submodules".to_string()
            }
            GitCommand::UpdateAllSubmodules => {
                git(&repo_path, ["submodule", "update"])?;
                "Updated all submodules".to_string()
            }
            GitCommand::UpdateAllSubmodulesRecursively => {
                git(&repo_path, ["submodule", "update", "--init", "--recursive"])?;
                "Updated all submodules recursively".to_string()
            }
            GitCommand::DeinitAllSubmodules => {
                git(&repo_path, ["submodule", "deinit", "--all", "--force"])?;
                "Deinitialized all submodules".to_string()
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
            GitCommand::UnsetBranchUpstream { branch_name } => {
                git(
                    &repo_path,
                    ["branch", "--unset-upstream", branch_name.as_str()],
                )?;
                format!("Unset upstream for {branch_name}")
            }
            GitCommand::FastForwardCurrentBranchFromUpstream { upstream_ref } => {
                git(&repo_path, ["merge", "--ff-only", upstream_ref.as_str()])?;
                format!("Fast-forwarded current branch from {upstream_ref}")
            }
            GitCommand::MergeRefIntoCurrent {
                target_ref,
                variant,
            } => {
                git(&repo_path, merge_ref_args(target_ref, *variant))?;
                format!(
                    "{} {target_ref} into current branch",
                    merged_summary_verb(*variant)
                )
            }
            GitCommand::RebaseCurrentOntoRef { target_ref } => {
                git(&repo_path, ["rebase", target_ref.as_str()])?;
                format!("Rebased current branch onto {target_ref}")
            }
            GitCommand::FetchRemote { remote_name } => {
                run_fetch_remote(&repo_path, remote_name)?;
                format!("Fetched {remote_name}")
            }
            GitCommand::UpdateBranchRefs { update_commands } => {
                update_branch_refs(&repo_path, update_commands)?;
                "Updated branch refs".to_string()
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
        GitCommand::StartBisect { .. } => "start_bisect",
        GitCommand::MarkBisect { .. } => "mark_bisect",
        GitCommand::SkipBisect { .. } => "skip_bisect",
        GitCommand::ResetBisect => "reset_bisect",
        GitCommand::CreateFixupCommit { .. } => "create_fixup_commit",
        GitCommand::CreateAmendCommit {
            include_file_changes,
            ..
        } => {
            if *include_file_changes {
                "create_amend_commit_with_changes"
            } else {
                "create_amend_commit_without_changes"
            }
        }
        GitCommand::AmendCommitAttributes {
            reset_author,
            co_author,
            ..
        } => match (*reset_author, co_author.is_some()) {
            (true, true) => "amend_commit_author_and_co_author",
            (true, false) => "amend_commit_reset_author",
            (false, true) => "amend_commit_set_co_author",
            (false, false) => "amend_commit_attributes",
        },
        GitCommand::RewordCommitWithEditor { .. } => "reword_commit_with_editor",
        GitCommand::StartCommitRebase { mode, .. } => match mode {
            RebaseStartMode::Interactive => "start_interactive_rebase",
            RebaseStartMode::Amend => "start_amend_rebase",
            RebaseStartMode::Fixup => "start_fixup_rebase",
            RebaseStartMode::FixupWithMessage => "set_fixup_message_rebase",
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
        GitCommand::StartGitFlow { branch_type, .. } => match branch_type {
            GitFlowBranchType::Feature => "start_git_flow_feature",
            GitFlowBranchType::Hotfix => "start_git_flow_hotfix",
            GitFlowBranchType::Bugfix => "start_git_flow_bugfix",
            GitFlowBranchType::Release => "start_git_flow_release",
        },
        GitCommand::AddRemote { .. } => "add_remote",
        GitCommand::CreateTag { .. } => "create_tag",
        GitCommand::CreateTagFromCommit { .. } => "create_tag_from_commit",
        GitCommand::CreateBranchFromCommit { .. } => "create_branch_from_commit",
        GitCommand::CreateBranchFromRef { .. } => "create_branch_from_ref",
        GitCommand::FinishGitFlow { .. } => "finish_git_flow",
        GitCommand::CreateBranchFromStash { .. } => "create_branch_from_stash",
        GitCommand::CheckoutBranch { .. } => "checkout_branch",
        GitCommand::ForceCheckoutRef { .. } => "force_checkout_ref",
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
        GitCommand::InitAllSubmodules => "init_all_submodules",
        GitCommand::UpdateAllSubmodules => "update_all_submodules",
        GitCommand::UpdateAllSubmodulesRecursively => "update_all_submodules_recursively",
        GitCommand::DeinitAllSubmodules => "deinit_all_submodules",
        GitCommand::RemoveSubmodule { .. } => "remove_submodule",
        GitCommand::SetBranchUpstream { .. } => "set_branch_upstream",
        GitCommand::UnsetBranchUpstream { .. } => "unset_branch_upstream",
        GitCommand::FastForwardCurrentBranchFromUpstream { .. } => {
            "fast_forward_current_branch_from_upstream"
        }
        GitCommand::MergeRefIntoCurrent { .. } => "merge_ref_into_current",
        GitCommand::RebaseCurrentOntoRef { .. } => "rebase_current_onto_ref",
        GitCommand::FetchRemote { .. } => "fetch_remote",
        GitCommand::UpdateBranchRefs { .. } => "update_branch_refs",
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

pub fn verify_in_git_repo(repo_path: &Path) -> GitResult<()> {
    RepoPaths::resolve(repo_path).map(|_| ())
}

fn is_git_repo(path: &Path) -> bool {
    git_stdout(path, ["rev-parse", "--show-toplevel"]).is_ok()
}

fn repo_path(repo_id: &RepoId) -> GitResult<PathBuf> {
    let repo_path = PathBuf::from(&repo_id.0);
    if verify_in_git_repo(&repo_path).is_err() {
        return Err(GitError::RepoNotFound {
            repo_id: repo_id.clone(),
        });
    }
    Ok(repo_path)
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct SyncPushOptions {
    force: bool,
    force_with_lease: bool,
    current_branch: String,
    upstream_remote: String,
    upstream_branch: String,
    set_upstream: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct SyncPullOptions {
    remote_name: String,
    branch_name: String,
    fast_forward_only: bool,
    worktree_git_dir: String,
    worktree_path: String,
}

fn build_push_command(opts: &SyncPushOptions) -> GitResult<GitCommandBuilder> {
    if !opts.upstream_branch.is_empty() && opts.upstream_remote.is_empty() {
        return Err(GitError::OperationFailed {
            message: "must specify origin when pushing to an explicit upstream branch".to_string(),
        });
    }

    let mut builder = GitCommandBuilder::new("push")
        .arg_if(opts.force, ["--force"])
        .arg_if(opts.force_with_lease, ["--force-with-lease"])
        .arg_if(opts.set_upstream, ["--set-upstream"]);

    if !opts.upstream_remote.is_empty() {
        builder = builder.arg([OsString::from(opts.upstream_remote.clone())]);
    }
    if !opts.upstream_branch.is_empty() {
        builder = builder.arg([OsString::from(format!(
            "refs/heads/{}:{}",
            opts.current_branch, opts.upstream_branch
        ))]);
    }

    Ok(builder)
}

fn fetch_command_builder(fetch_all: bool) -> GitCommandBuilder {
    GitCommandBuilder::new("fetch")
        .arg_if(fetch_all, ["--all"])
        .arg(["--no-write-fetch-head"])
}

fn build_pull_command(opts: &SyncPullOptions) -> GitCommandBuilder {
    let mut builder = GitCommandBuilder::new("pull")
        .arg(["--no-edit"])
        .arg_if(opts.fast_forward_only, ["--ff-only"]);

    if !opts.remote_name.is_empty() {
        builder = builder.arg([OsString::from(opts.remote_name.clone())]);
    }
    if !opts.branch_name.is_empty() {
        builder = builder.arg([OsString::from(format!("refs/heads/{}", opts.branch_name))]);
    }

    builder
        .worktree_path_if(!opts.worktree_path.is_empty(), opts.worktree_path.clone())
        .git_dir_if(
            !opts.worktree_git_dir.is_empty(),
            opts.worktree_git_dir.clone(),
        )
}

fn run_fetch(repo_path: &Path) -> GitResult<()> {
    if let Some(remote) = default_remote(repo_path)? {
        run_fetch_remote(repo_path, remote.as_str())
    } else {
        git_builder(repo_path, fetch_command_builder(true))?;
        auto_forward_default_branches(repo_path)
    }
}

fn run_fetch_remote(repo_path: &Path, remote_name: &str) -> GitResult<()> {
    git_builder(
        repo_path,
        fetch_command_builder(false).arg([OsString::from(remote_name)]),
    )?;
    auto_forward_default_branches(repo_path)
}

fn run_pull(repo_path: &Path) -> GitResult<()> {
    if has_upstream(repo_path)? {
        git_builder_with_env(
            repo_path,
            build_pull_command(&SyncPullOptions {
                fast_forward_only: true,
                ..SyncPullOptions::default()
            }),
            &[("GIT_SEQUENCE_EDITOR", OsStr::new(":"))],
        )
    } else {
        Err(GitError::OperationFailed {
            message: "pull requires an upstream tracking branch".to_string(),
        })
    }
}

fn run_push(repo_path: &Path) -> GitResult<()> {
    let builder = if has_upstream(repo_path)? {
        build_push_command(&SyncPushOptions::default())?
    } else {
        let branch = current_branch_name(repo_path)?;
        let remote = default_remote(repo_path)?.unwrap_or_else(|| "origin".to_string());
        build_push_command(&SyncPushOptions {
            current_branch: branch.clone(),
            upstream_remote: remote,
            upstream_branch: branch,
            set_upstream: true,
            ..SyncPushOptions::default()
        })?
    };

    git_builder(repo_path, builder)
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

fn current_branch_list_entry(repo_path: &Path) -> Option<String> {
    let symbolic_ref =
        git_stdout_allow_failure(repo_path, ["symbolic-ref", "--short", "HEAD"]).ok()?;
    let detached = git_stdout_allow_failure(
        repo_path,
        [
            "branch",
            "--points-at=HEAD",
            "--format=%(HEAD)%00%(objectname:short)%00%(refname)",
        ],
    )
    .ok()?;

    Some(parse_current_branch_list_entry(
        symbolic_ref.trim(),
        detached.as_str(),
    ))
}

fn parse_current_branch_list_entry(symbolic_ref: &str, detached_output: &str) -> String {
    if !symbolic_ref.is_empty() && symbolic_ref != "HEAD" {
        return symbolic_ref.to_string();
    }

    if let Some(entry) = parse_detached_head_entry(detached_output) {
        return entry;
    }

    "HEAD".to_string()
}

fn parse_detached_head_entry(detached_output: &str) -> Option<String> {
    for line in detached_output.lines() {
        let parts: Vec<_> = line.trim_end_matches(['\r', '\n']).split('\0').collect();
        if parts.len() != 3 || parts[0].trim() != "*" {
            continue;
        }

        let short_oid = parts[1].trim();
        let display_name = parts[2].trim();
        if !display_name.is_empty() {
            return Some(display_name.to_string());
        }
        if !short_oid.is_empty() {
            return Some(format!("(HEAD detached at {short_oid})"));
        }
    }

    None
}

fn fetch_head_timestamp(repo_path: &Path) -> GitResult<Option<Timestamp>> {
    let fetch_head = RepoPaths::resolve(repo_path)?.git_dir().join("FETCH_HEAD");
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

fn delete_branch_args(branch_name: &str, force: bool) -> Vec<OsString> {
    vec![
        OsString::from("branch"),
        OsString::from(if force { "-D" } else { "-d" }),
        OsString::from(branch_name),
    ]
}

fn create_branch_from_ref_args(branch_name: &str, start_point: &str, track: bool) -> Vec<OsString> {
    vec![
        OsString::from("branch"),
        OsString::from(if track { "--track" } else { "--no-track" }),
        OsString::from(branch_name),
        OsString::from(start_point),
    ]
}

fn git_flow_start_args(branch_type: GitFlowBranchType, name: &str) -> Vec<OsString> {
    vec![
        OsString::from("flow"),
        OsString::from(branch_type.command_name()),
        OsString::from("start"),
        OsString::from(name),
    ]
}

fn git_flow_finish_args(branch_type: &str, suffix: &str) -> Vec<OsString> {
    vec![
        OsString::from("flow"),
        OsString::from(branch_type),
        OsString::from("finish"),
        OsString::from(suffix),
    ]
}

fn resolve_git_flow_finish_parts(
    repo_path: &Path,
    branch_name: &str,
) -> GitResult<(String, String)> {
    let prefixes = git_flow_prefixes(repo_path);
    let (prefix, suffix) = git_flow_branch_prefix_and_suffix(branch_name);
    for line in prefixes
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Some(config) = line.strip_prefix("gitflow.prefix.") else {
            continue;
        };
        let Some((branch_type, configured_prefix)) = config.split_once(char::is_whitespace) else {
            continue;
        };
        if configured_prefix.trim() == prefix {
            return Ok((branch_type.to_string(), suffix));
        }
    }
    Err(GitError::OperationFailed {
        message: "This does not seem to be a git flow branch".to_string(),
    })
}

fn git_flow_branch_prefix_and_suffix(branch_name: &str) -> (String, String) {
    match branch_name.split_once('/') {
        Some((prefix, suffix)) => (format!("{prefix}/"), suffix.to_string()),
        None => (branch_name.to_string(), String::new()),
    }
}

type GitConfigRunner = dyn Fn(&Path, &[OsString]) -> GitResult<String> + Send + Sync;

struct CachedGitConfig {
    cache: Mutex<HashMap<String, String>>,
    repo_path: PathBuf,
    run_git_config_cmd: Arc<GitConfigRunner>,
}

impl CachedGitConfig {
    fn new(repo_path: &Path) -> Self {
        Self::with_runner(repo_path, run_git_config_cmd)
    }

    fn with_runner(
        repo_path: &Path,
        run_git_config_cmd: impl Fn(&Path, &[OsString]) -> GitResult<String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            repo_path: canonicalize_existing_path(repo_path),
            run_git_config_cmd: Arc::new(run_git_config_cmd),
        }
    }

    fn get(&self, key: &str) -> String {
        let mut cache = self
            .cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(value) = cache.get(key) {
            return value.clone();
        }

        let value = self.get_aux(key);
        cache.insert(key.to_string(), value.clone());
        value
    }

    fn get_general(&self, args: &str) -> String {
        let mut cache = self
            .cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(value) = cache.get(args) {
            return value.clone();
        }

        let value = self.get_general_aux(args);
        cache.insert(args.to_string(), value.clone());
        value
    }

    #[allow(dead_code)]
    fn get_bool(&self, key: &str) -> bool {
        is_truthy(&self.get(key))
    }

    #[allow(dead_code)]
    fn drop_cache(&self) {
        self.cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
    }

    fn get_aux(&self, key: &str) -> String {
        self.run_git_config(get_git_config_args(key))
            .map(|value| value.trim().to_string())
            .unwrap_or_default()
    }

    fn get_general_aux(&self, args: &str) -> String {
        self.run_git_config(get_git_config_general_args(args))
            .map(|value| value.trim().to_string())
            .unwrap_or_default()
    }

    fn run_git_config(&self, args: Vec<OsString>) -> GitResult<String> {
        (self.run_git_config_cmd)(self.repo_path.as_path(), &args)
    }
}

fn run_git_config_cmd(repo_path: &Path, args: &[OsString]) -> GitResult<String> {
    let output = git_output(repo_path, args.iter())?;
    if !output.status.success() {
        return Err(command_failure(output));
    }
    stdout_string(output).map(|value| value.trim_end_matches('\0').to_string())
}

fn get_git_config_args(key: &str) -> Vec<OsString> {
    vec![
        OsString::from("config"),
        OsString::from("--get"),
        OsString::from("--null"),
        OsString::from(key),
    ]
}

fn get_git_config_general_args(args: &str) -> Vec<OsString> {
    std::iter::once(OsString::from("config"))
        .chain(args.split(' ').map(OsString::from))
        .collect()
}

#[allow(dead_code)]
fn is_truthy(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

fn git_flow_prefixes(repo_path: &Path) -> String {
    CachedGitConfig::new(repo_path).get_general("--local --get-regexp gitflow.prefix")
}

fn merge_ref_args(target_ref: &str, variant: MergeVariant) -> Vec<OsString> {
    let mut args = vec![OsString::from("merge"), OsString::from("--no-edit")];
    match variant {
        MergeVariant::Regular => {}
        MergeVariant::FastForward => args.push(OsString::from("--ff")),
        MergeVariant::NoFastForward => args.push(OsString::from("--no-ff")),
        MergeVariant::Squash => {
            args.push(OsString::from("--squash"));
            args.push(OsString::from("--ff"));
        }
    }
    args.push(OsString::from(target_ref));
    args
}

fn merged_summary_verb(variant: MergeVariant) -> &'static str {
    match variant {
        MergeVariant::Regular | MergeVariant::FastForward | MergeVariant::NoFastForward => "Merged",
        MergeVariant::Squash => "Squash-merged",
    }
}

fn is_branch_merged(repo_path: &Path, branch_name: &str) -> GitResult<bool> {
    let mut refs = vec![String::from("HEAD")];
    if branch_has_upstream(repo_path, branch_name)? {
        refs.push(format!("{branch_name}@{{upstream}}"));
    }
    refs.extend(existing_main_branch_refs(repo_path)?);

    let mut args = vec![
        OsString::from("rev-list"),
        OsString::from("--max-count=1"),
        OsString::from(branch_name),
    ];
    args.extend(
        refs.into_iter()
            .map(|reference| OsString::from(format!("^{reference}"))),
    );
    args.push(OsString::from("--"));

    Ok(git_stdout(repo_path, args)?.trim().is_empty())
}

fn can_do_fast_forward_merge(repo_path: &Path, ref_name: &str) -> bool {
    git_output(repo_path, ["merge-base", "--is-ancestor", "HEAD", ref_name])
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn branch_has_upstream(repo_path: &Path, branch_name: &str) -> GitResult<bool> {
    let upstream = format!("{branch_name}@{{u}}");
    Ok(git_output(
        repo_path,
        ["rev-parse", "--symbolic-full-name", upstream.as_str()],
    )?
    .status
    .success())
}

fn existing_main_branch_refs(repo_path: &Path) -> GitResult<Vec<String>> {
    let mut refs = Vec::new();
    let mut seen = HashSet::new();

    for branch_name in DEFAULT_MAIN_BRANCH_NAMES {
        if let Some(reference) = resolve_main_branch_ref(repo_path, branch_name)? {
            if seen.insert(reference.clone()) {
                refs.push(reference);
            }
        }
    }

    Ok(refs)
}

#[allow(dead_code)]
fn branch_base_reference(repo_path: &Path, branch: &BranchItem) -> GitResult<Option<String>> {
    let main_branches = existing_main_branch_refs(repo_path)?;
    if main_branches.is_empty() {
        return Ok(None);
    }

    let branch_ref = if branch.detached_head {
        branch.name.clone()
    } else {
        format!("refs/heads/{}", branch.name)
    };
    let mut args = vec![OsString::from("merge-base"), OsString::from(branch_ref)];
    args.extend(main_branches.iter().map(OsString::from));
    let merge_base = git_stdout_allow_failure(repo_path, args)?;
    let merge_base = merge_base.trim();
    if merge_base.is_empty() {
        return Ok(None);
    }

    let mut args = vec![
        OsString::from("for-each-ref"),
        OsString::from("--contains"),
        OsString::from(merge_base),
        OsString::from("--format=%(refname)"),
    ];
    args.extend(main_branches.iter().map(OsString::from));
    let output = git_stdout_allow_failure(repo_path, args)?;
    Ok(output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToOwned::to_owned))
}

#[allow(dead_code)]
fn branch_behind_base_count(
    repo_path: &Path,
    branch: &BranchItem,
    base_branch: &str,
) -> GitResult<i32> {
    let branch_ref = if branch.detached_head {
        branch.name.clone()
    } else {
        format!("refs/heads/{}", branch.name)
    };
    let output = git_stdout_allow_failure(
        repo_path,
        [
            "rev-list",
            "--left-right",
            "--count",
            format!("{branch_ref}...{base_branch}").as_str(),
        ],
    )?;
    let mut counts = output.trim().split('\t');
    let _ahead = counts.next();
    Ok(counts
        .next()
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(0))
}

fn resolve_main_branch_ref(repo_path: &Path, branch_name: &str) -> GitResult<Option<String>> {
    let upstream = format!("{branch_name}@{{u}}");
    if let Ok(reference) = git_stdout(
        repo_path,
        ["rev-parse", "--symbolic-full-name", upstream.as_str()],
    ) {
        let reference = reference.trim().to_string();
        if !reference.is_empty() {
            return Ok(Some(reference));
        }
    }

    for reference in [
        format!("refs/remotes/origin/{branch_name}"),
        format!("refs/heads/{branch_name}"),
    ] {
        if git_output(
            repo_path,
            ["rev-parse", "--verify", "--quiet", reference.as_str()],
        )?
        .status
        .success()
        {
            return Ok(Some(reference));
        }
    }

    Ok(None)
}

fn read_merge_fast_forward_preference(repo_path: &Path) -> MergeFastForwardPreference {
    let value = CachedGitConfig::new(repo_path).get("merge.ff");
    match value.as_str() {
        "true" | "only" => MergeFastForwardPreference::FastForward,
        "false" => MergeFastForwardPreference::NoFastForward,
        _ => MergeFastForwardPreference::Default,
    }
}

fn read_fast_forward_merge_targets(
    repo_path: &Path,
    branches: &[BranchItem],
    remote_branches: &[RemoteBranchItem],
) -> BTreeMap<String, bool> {
    let mut targets = BTreeMap::new();

    for reference in branches
        .iter()
        .filter(|branch| !branch.is_head)
        .map(|branch| branch.name.as_str())
        .chain(remote_branches.iter().map(|branch| branch.name.as_str()))
    {
        targets
            .entry(reference.to_string())
            .or_insert_with(|| can_do_fast_forward_merge(repo_path, reference));
    }

    targets
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AutoForwardBranchCandidate {
    name: String,
    full_ref: String,
    upstream_full_ref: String,
    commit_hash: String,
    is_head: bool,
}

fn auto_forward_default_branches(repo_path: &Path) -> GitResult<()> {
    let update_commands = collect_auto_forward_branch_updates(repo_path)?;
    update_branch_refs(repo_path, &update_commands)
}

fn collect_auto_forward_branch_updates(repo_path: &Path) -> GitResult<String> {
    let checked_out_branch_refs = read_checked_out_branch_refs(repo_path)?;
    let mut update_commands = String::new();

    for branch in read_auto_forward_branch_candidates(repo_path)? {
        if branch.is_head
            || branch.upstream_full_ref.is_empty()
            || !DEFAULT_MAIN_BRANCH_NAMES.contains(&branch.name.as_str())
            || checked_out_branch_refs.contains(branch.full_ref.as_str())
            || !ref_exists(repo_path, branch.upstream_full_ref.as_str())?
        {
            continue;
        }

        let (ahead, behind) = branch_divergence_counts(
            repo_path,
            branch.full_ref.as_str(),
            branch.upstream_full_ref.as_str(),
        )?;
        if behind > 0 && ahead == 0 {
            update_commands.push_str(
                format!(
                    "update {} {} {}\n",
                    branch.full_ref, branch.upstream_full_ref, branch.commit_hash
                )
                .as_str(),
            );
        }
    }

    Ok(update_commands)
}

fn read_auto_forward_branch_candidates(
    repo_path: &Path,
) -> GitResult<Vec<AutoForwardBranchCandidate>> {
    git_stdout(
        repo_path,
        [
            "for-each-ref",
            "--format=%(HEAD)%00%(refname:short)%00%(refname)%00%(upstream)%00%(objectname)",
            "refs/heads",
        ],
    )
    .map(|output| {
        output
            .lines()
            .filter_map(parse_auto_forward_branch_candidate)
            .collect()
    })
}

fn parse_auto_forward_branch_candidate(line: &str) -> Option<AutoForwardBranchCandidate> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut parts = trimmed.split('\0');
    let head = parts.next().unwrap_or_default().trim();
    let name = normalize_local_branch_name(parts.next().unwrap_or_default().trim());
    let full_ref = parts.next().unwrap_or_default().trim().to_string();
    let upstream_full_ref = parts.next().unwrap_or_default().trim().to_string();
    let commit_hash = parts.next().unwrap_or_default().trim().to_string();

    if name.is_empty() || full_ref.is_empty() || commit_hash.is_empty() {
        return None;
    }

    Some(AutoForwardBranchCandidate {
        name: name.to_string(),
        full_ref,
        upstream_full_ref,
        commit_hash,
        is_head: head == "*",
    })
}

fn read_checked_out_branch_refs(repo_path: &Path) -> GitResult<HashSet<String>> {
    Ok(
        git_stdout_allow_failure(repo_path, ["worktree", "list", "--porcelain"])?
            .lines()
            .filter_map(|line| line.strip_prefix("branch "))
            .map(str::trim)
            .filter(|branch_ref| !branch_ref.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

fn branch_divergence_counts(
    repo_path: &Path,
    local_ref: &str,
    upstream_ref: &str,
) -> GitResult<(usize, usize)> {
    let refspec = format!("{local_ref}...{upstream_ref}");
    let counts = git_stdout(
        repo_path,
        ["rev-list", "--left-right", "--count", refspec.as_str()],
    )?;
    let mut parts = counts.split_whitespace();
    let ahead = parse_divergence_count(parts.next())?;
    let behind = parse_divergence_count(parts.next())?;
    Ok((ahead, behind))
}

fn parse_divergence_count(raw: Option<&str>) -> GitResult<usize> {
    raw.unwrap_or_default()
        .parse::<usize>()
        .map_err(|error| GitError::OperationFailed {
            message: format!("failed to parse branch divergence count: {error}"),
        })
}

fn ref_exists(repo_path: &Path, reference: &str) -> GitResult<bool> {
    Ok(
        git_output(repo_path, ["rev-parse", "--verify", "--quiet", reference])?
            .status
            .success(),
    )
}

fn update_branch_refs(repo_path: &Path, update_commands: &str) -> GitResult<()> {
    if update_commands.trim().is_empty() {
        return Ok(());
    }
    git_with_stdin(
        repo_path,
        ["update-ref", "--stdin"],
        update_commands.as_bytes(),
    )
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
    let args: Vec<OsString> = args
        .into_iter()
        .map(|arg| arg.as_ref().to_os_string())
        .collect();

    for _ in 0..GIT_INDEX_LOCK_RETRY_COUNT.saturating_sub(1) {
        let output = run_git_output(repo_path, &args, envs)?;
        if !should_retry_git_output(&output) {
            return Ok(output);
        }
        std::thread::sleep(GIT_INDEX_LOCK_RETRY_WAIT);
    }

    run_git_output(repo_path, &args, envs)
}

fn run_git_output(
    repo_path: &Path,
    args: &[OsString],
    envs: &[(&str, &OsStr)],
) -> GitResult<Output> {
    Command::new("git")
        .args(args)
        .env(GIT_OPTIONAL_LOCKS_ENV, GIT_OPTIONAL_LOCKS_DISABLED)
        .envs(envs.iter().copied())
        .current_dir(repo_path)
        .output()
        .map_err(io_error)
}

fn should_retry_git_output(output: &Output) -> bool {
    !output.status.success()
        && (String::from_utf8_lossy(&output.stdout).contains(GIT_INDEX_LOCK_MARKER)
            || String::from_utf8_lossy(&output.stderr).contains(GIT_INDEX_LOCK_MARKER))
}

fn git_builder_output(repo_path: &Path, builder: GitCommandBuilder) -> GitResult<Output> {
    git_output(repo_path, builder.into_args())
}

fn git_builder_output_with_env(
    repo_path: &Path,
    builder: GitCommandBuilder,
    envs: &[(&'static str, &OsStr)],
) -> GitResult<Output> {
    git_output_with_env(repo_path, builder.into_args(), envs)
}

fn git_builder(repo_path: &Path, builder: GitCommandBuilder) -> GitResult<()> {
    git_builder_with_env(repo_path, builder, &[])
}

fn git_builder_with_env(
    repo_path: &Path,
    builder: GitCommandBuilder,
    envs: &[(&'static str, &OsStr)],
) -> GitResult<()> {
    let output = git_builder_output_with_env(repo_path, builder, envs)?;
    if !output.status.success() {
        return Err(command_failure(output));
    }
    Ok(())
}

fn build_git_path_command<I, S>(args: I, path: &Path) -> GitResult<GitCommandBuilder>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut args = args.into_iter();
    let command = args
        .next()
        .ok_or_else(|| GitError::OperationFailed {
            message: "git path command requires at least one argument".to_string(),
        })?
        .as_ref()
        .to_os_string();

    Ok(GitCommandBuilder::new(command)
        .arg(args.map(|arg| arg.as_ref().to_os_string()))
        .arg(["--"])
        .arg([path.as_os_str().to_os_string()]))
}

fn git_path<I, S>(repo_path: &Path, args: I, path: &Path) -> GitResult<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = git_builder_output(repo_path, build_git_path_command(args, path)?)?;
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
    git_builder_output(repo_path, build_git_path_command(args, path)?)
}

#[allow(dead_code)]
fn run_git_cmd_on_paths(repo_path: &Path, subcommand: &str, paths: &[PathBuf]) -> GitResult<()> {
    run_git_cmd_on_paths_with_runner(
        subcommand,
        paths.iter().map(|path| path.as_os_str().to_os_string()),
        |builder| git_builder(repo_path, builder),
    )
}

#[allow(dead_code)]
fn run_git_cmd_on_paths_with_runner<I, S, F>(
    subcommand: &str,
    paths: I,
    mut run_builder: F,
) -> GitResult<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
    F: FnMut(GitCommandBuilder) -> GitResult<()>,
{
    const MAX_ARG_BYTES: usize = 30_000;

    let paths: Vec<OsString> = paths
        .into_iter()
        .map(|path| path.as_ref().to_os_string())
        .collect();

    let mut start = 0;
    while start < paths.len() {
        let mut end = start;
        let mut total = 0;
        while end < paths.len() {
            total += paths[end].as_os_str().as_encoded_bytes().len() + 1;
            if total > MAX_ARG_BYTES && end > start {
                break;
            }
            end += 1;
        }

        run_builder(
            GitCommandBuilder::new(subcommand)
                .arg(["--"])
                .arg(paths[start..end].iter().cloned()),
        )?;
        start = end;
    }

    Ok(())
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

#[allow(dead_code)]
fn git_blame_line_range(
    repo_path: &Path,
    filename: &Path,
    commit: &str,
    first_line: usize,
    num_lines: usize,
) -> GitResult<String> {
    let output = git_path_output(
        repo_path,
        [
            OsStr::new("blame"),
            OsStr::new("-l"),
            OsStr::new(&format!("-L{first_line},+{num_lines}")),
            OsStr::new(commit),
        ],
        filename,
    )?;
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

fn stash_staged_changes(repo_path: &Path, message: Option<&str>) -> GitResult<()> {
    if git_version_at_least(repo_path, 2, 35, 0)? {
        return stash_push(repo_path, &["--staged"], message);
    }

    stash_staged_changes_legacy(repo_path, message)
}

fn stash_staged_changes_legacy(repo_path: &Path, message: Option<&str>) -> GitResult<()> {
    git(repo_path, ["stash", "--keep-index"])?;
    stash_push(repo_path, &[], message)?;
    git(repo_path, ["stash", "apply", "refs/stash@{1}"])?;

    let staged_patch = git_stdout_raw(repo_path, ["stash", "show", "-p"])?;
    git_with_stdin(repo_path, ["apply", "-R"], staged_patch.as_bytes())?;
    git(repo_path, ["stash", "drop", "refs/stash@{1}"])?;

    cleanup_staged_added_deleted_entries(repo_path)
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

fn cleanup_staged_added_deleted_entries(repo_path: &Path) -> GitResult<()> {
    let status = git_stdout_raw(
        repo_path,
        ["status", "--porcelain", "-z", "--untracked-files=all"],
    )?;
    let parsed = parse_status(&status);

    for file in parsed
        .file_tree
        .iter()
        .filter(|file| file.short_status == "AD")
    {
        unstage_path(repo_path, &file.path)?;
    }

    Ok(())
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

fn git_version_at_least(repo_path: &Path, major: u32, minor: u32, patch: u32) -> GitResult<bool> {
    Ok(parse_git_version(&git_stdout(repo_path, ["version"])?)? >= (major, minor, patch))
}

fn parse_git_version(raw: &str) -> GitResult<(u32, u32, u32)> {
    let Some(token) = raw.split_whitespace().find(|token| {
        token
            .chars()
            .next()
            .is_some_and(|char| char.is_ascii_digit())
    }) else {
        return Err(GitError::OperationFailed {
            message: format!("failed to parse git version from {raw:?}"),
        });
    };

    let mut numbers = Vec::new();
    for segment in token.split('.') {
        let Ok(number) = segment.parse::<u32>() else {
            break;
        };
        numbers.push(number);
        if numbers.len() == 3 {
            break;
        }
    }

    if numbers.len() < 2 {
        return Err(GitError::OperationFailed {
            message: format!("failed to parse git version from {raw:?}"),
        });
    }

    Ok((numbers[0], numbers[1], numbers.get(2).copied().unwrap_or(0)))
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
    process_failure("git", output)
}

fn process_failure(command_name: &str, output: Output) -> GitError {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    GitError::OperationFailed {
        message: format!(
            "{command_name} exited with status {}\nstdout:\n{}\nstderr:\n{}",
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
        RebaseStartMode::FixupWithMessage => {
            run_scripted_rebase(repo_path, commit, "fixup -C", None, false)
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

fn amend_commit_attributes(
    repo_path: &Path,
    commit: &str,
    reset_author: bool,
    co_author: Option<&str>,
) -> GitResult<()> {
    let resolved_commit = git_stdout(repo_path, ["rev-parse", commit])?;
    let head_commit = git_stdout(repo_path, ["rev-parse", "HEAD"])?;
    if resolved_commit == head_commit {
        amend_current_commit_attributes(repo_path, reset_author, co_author)?;
    } else {
        run_scripted_rebase(repo_path, &resolved_commit, "edit", None, false)?;
        amend_current_commit_attributes(repo_path, reset_author, co_author)?;
        git_with_env(
            repo_path,
            ["rebase", "--continue"],
            &[("GIT_EDITOR", OsStr::new(":"))],
        )?;
    }
    Ok(())
}

fn amend_current_commit_attributes(
    repo_path: &Path,
    reset_author: bool,
    co_author: Option<&str>,
) -> GitResult<()> {
    let mut args = vec![
        "commit".to_string(),
        "--amend".to_string(),
        "--allow-empty".to_string(),
        "--allow-empty-message".to_string(),
        "--only".to_string(),
        "--no-edit".to_string(),
    ];
    if reset_author {
        args.push("--reset-author".to_string());
    }
    if let Some(co_author) = co_author {
        args.push("--trailer".to_string());
        args.push(co_author.to_string());
    }
    git(repo_path, args)
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
    let sequence_editor_command = git_script_command(&sequence_editor);
    let editor_command = if let Some(_message) = reword_message {
        write_executable_script(
            &editor_path,
            "#!/bin/sh\nset -eu\nprintf '%s\\n' \"$SUPER_LAZYGIT_REWORD\" > \"$1\"\n",
        )?;
        Some(git_script_command(&editor_path))
    } else {
        None
    };
    let mut envs: Vec<(&str, &OsStr)> =
        vec![("GIT_SEQUENCE_EDITOR", sequence_editor_command.as_os_str())];

    if let Some(message) = reword_message {
        let editor_command = editor_command
            .as_ref()
            .expect("editor command should exist when rewording");
        envs.push(("GIT_EDITOR", editor_command.as_os_str()));
        envs.push(("SUPER_LAZYGIT_REWORD", OsStr::new(message)));
    } else {
        envs.push(("GIT_EDITOR", OsStr::new(":")));
    }

    let mut args = vec!["rebase".to_string(), "-i".to_string()];
    if autosquash {
        args.push("--autosquash".to_string());
    }
    if todo_verb == "squash" || todo_verb.starts_with("fixup") {
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

    let sequence_editor_command = git_script_command(&sequence_editor);
    let envs: Vec<(&str, &OsStr)> = vec![
        ("GIT_SEQUENCE_EDITOR", sequence_editor_command.as_os_str()),
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

fn git_script_command(path: &Path) -> OsString {
    #[cfg(windows)]
    {
        OsString::from(format!("sh {}", path.to_string_lossy().replace('\\', "/")))
    }

    #[cfg(not(windows))]
    {
        path.as_os_str().to_os_string()
    }
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

fn read_status_snapshot(
    repo_path: &Path,
    rename_similarity_threshold: u8,
) -> GitResult<ParsedStatus> {
    let status = git_stdout_raw(
        repo_path,
        [
            "status".to_string(),
            "--branch".to_string(),
            "--porcelain".to_string(),
            "-z".to_string(),
            "--untracked-files=all".to_string(),
            format!("--find-renames={}%", rename_similarity_threshold),
        ],
    )?;
    let mut parsed = parse_status(&status);
    let file_diffs = get_file_diffs(repo_path)?;
    enrich_status_with_numstat(&mut parsed.file_tree, &file_diffs);
    mark_worktree_entries(repo_path, &mut parsed.file_tree, &read_worktrees(repo_path));
    parsed.first_path = parsed.file_tree.first().map(|item| item.path.clone());
    Ok(parsed)
}

fn parse_status(status: &str) -> ParsedStatus {
    let mut parsed = ParsedStatus::default();
    let records: Vec<&str> = status.split('\0').collect();
    let mut index = 0;

    while index < records.len() {
        let record = records[index];
        index += 1;

        if record.is_empty() {
            continue;
        }
        if let Some(branch_line) = record.strip_prefix("## ") {
            parse_branch_header(branch_line, &mut parsed);
            continue;
        }
        if record.starts_with("warning") {
            continue;
        }

        let bytes = record.as_bytes();
        if bytes.len() < 3 {
            continue;
        }

        let staged = bytes[0] as char;
        let unstaged = bytes[1] as char;
        let change = &record[..2];
        let path = status_path(&record[3..]);
        if path.starts_with(".super-lazygit") {
            continue;
        }
        let mut previous_path = None;
        let mut display_string = record.to_string();

        if matches!(staged, 'R' | 'C') {
            if let Some(previous) = records.get(index).copied().filter(|item| !item.is_empty()) {
                previous_path = Some(status_path(previous));
                display_string = format!("{change} {previous} -> {}", path.display());
                index += 1;
            }
        }

        if staged == '?' && unstaged == '?' {
            parsed.untracked_count += 1;
            parsed.file_tree.push(FileStatus {
                path: path.clone(),
                previous_path,
                kind: FileStatusKind::Untracked,
                staged_kind: None,
                unstaged_kind: Some(FileStatusKind::Untracked),
                short_status: change.to_string(),
                display_string,
                ..FileStatus::default()
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
            previous_path,
            kind: status_kind(staged, unstaged),
            staged_kind: staged_status_kind(staged, unstaged),
            unstaged_kind: unstaged_status_kind(staged, unstaged),
            short_status: change.to_string(),
            display_string,
            ..FileStatus::default()
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
    PathBuf::from(raw.trim().trim_matches('"'))
}

fn get_file_diffs(repo_path: &Path) -> GitResult<BTreeMap<PathBuf, (u32, u32)>> {
    let output = git_output(repo_path, ["diff", "--numstat", "-z", "HEAD"])?;
    if !output.status.success() {
        return Ok(BTreeMap::new());
    }
    let diffs = stdout_raw_string(output)?;
    Ok(parse_numstat(&diffs))
}

fn parse_numstat(diffs: &str) -> BTreeMap<PathBuf, (u32, u32)> {
    let records: Vec<&str> = diffs.split('\0').collect();
    let mut parsed = BTreeMap::new();
    let mut index = 0;

    while index < records.len() {
        let record = records[index];
        index += 1;

        if record.is_empty() {
            continue;
        }

        let mut parts = record.split('\t');
        let Some(lines_added) = parts.next().and_then(|value| value.parse::<u32>().ok()) else {
            continue;
        };
        let Some(lines_deleted) = parts.next().and_then(|value| value.parse::<u32>().ok()) else {
            continue;
        };
        let Some(path_part) = parts.next() else {
            continue;
        };

        if path_part.is_empty() {
            let Some(new_path) = records
                .get(index + 1)
                .copied()
                .filter(|item| !item.is_empty())
            else {
                continue;
            };
            parsed.insert(PathBuf::from(new_path), (lines_added, lines_deleted));
            index += 2;
            continue;
        }

        parsed.insert(PathBuf::from(path_part), (lines_added, lines_deleted));
    }

    parsed
}

fn enrich_status_with_numstat(
    file_tree: &mut [FileStatus],
    file_diffs: &BTreeMap<PathBuf, (u32, u32)>,
) {
    for item in file_tree {
        if let Some((lines_added, lines_deleted)) = file_diffs.get(&item.path) {
            item.lines_added = *lines_added;
            item.lines_deleted = *lines_deleted;
        }
    }
}

fn mark_worktree_entries(
    repo_path: &Path,
    file_tree: &mut [FileStatus],
    worktrees: &[WorktreeItem],
) {
    let worktree_paths: HashSet<PathBuf> = worktrees
        .iter()
        .map(|item| normalized_worktree_path(&item.path))
        .collect();

    for item in file_tree {
        let absolute_path = normalized_worktree_path(&repo_path.join(&item.path));
        if worktree_paths.contains(&absolute_path) {
            item.is_worktree = true;
            item.path = trimmed_status_path(&item.path);
        }
    }
}

fn normalized_worktree_path(path: &Path) -> PathBuf {
    trimmed_status_path(path)
}

fn trimmed_status_path(path: &Path) -> PathBuf {
    let value = path.to_string_lossy();
    let trimmed = value.trim_end_matches(['/', '\\']);
    if trimmed.is_empty() {
        path.to_path_buf()
    } else {
        PathBuf::from(trimmed)
    }
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
        'R' | 'C' => FileStatusKind::Renamed,
        '?' => FileStatusKind::Untracked,
        ' ' => return None,
        _ => FileStatusKind::Modified,
    })
}

fn read_branches(repo_path: &Path) -> Vec<BranchItem> {
    let branch_configs = read_branch_configs(repo_path);
    git_stdout(
        repo_path,
        [
            "for-each-ref",
            "--sort=refname",
            "--format=%(HEAD)%00%(refname:short)%00%(upstream:short)%00%(upstream:track)%00%(push:track)%00%(subject)%00%(objectname)%00%(committerdate:unix)",
            "refs/heads",
        ],
    )
    .map(|output| {
        let mut branches: Vec<_> = output
            .lines()
            .filter_map(|line| parse_branch_line(line, &branch_configs))
            .collect();

        if let Some(head_index) = branches.iter().position(|branch| branch.is_head) {
            let mut head_branch = branches.remove(head_index);
            head_branch.recency = "  *".to_string();
            branches.insert(0, head_branch);
        } else if let Some(current_ref) = current_branch_list_entry(repo_path) {
            branches.insert(
                0,
                BranchItem {
                    name: current_ref,
                    display_name: None,
                    is_head: true,
                    detached_head: true,
                    upstream: None,
                    recency: "  *".to_string(),
                    ..BranchItem::default()
                },
            );
        }

        branches
    })
    .unwrap_or_default()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct BranchConfig {
    remote: Option<String>,
    merge: Option<String>,
}

fn parse_branch_line(
    line: &str,
    branch_configs: &BTreeMap<String, BranchConfig>,
) -> Option<BranchItem> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let parts: Vec<_> = trimmed.split('\0').collect();
    if parts.len() != 8 {
        return None;
    }

    let head = parts[0].trim();
    let name = normalize_local_branch_name(parts[1].trim());
    let upstream = parts[2].trim();
    let track = parts[3].trim();
    let push_track = parts[4].trim();
    let subject = parts[5].trim();
    let commit_hash = parts[6].trim();
    let commit_timestamp = parse_branch_commit_timestamp(parts[7].trim());

    if name.is_empty() {
        return None;
    }

    let config = branch_configs.get(name);
    let upstream_remote = config
        .and_then(|config| config.remote.clone())
        .or_else(|| parse_upstream_remote(upstream));
    let upstream_branch = config
        .and_then(|config| config.merge.as_deref())
        .map(normalize_local_branch_name)
        .map(ToOwned::to_owned)
        .or_else(|| parse_upstream_branch(upstream));
    let upstream = if upstream.is_empty() {
        upstream_remote
            .as_deref()
            .zip(upstream_branch.as_deref())
            .map(|(remote, branch)| format!("{remote}/{branch}"))
    } else {
        Some(upstream.to_string())
    };
    let (ahead_for_pull, behind_for_pull, upstream_gone) = parse_upstream_info(
        (!parts[2].trim().is_empty()).then_some(parts[2].trim()),
        track,
    );
    let (ahead_for_push, behind_for_push, _) = parse_upstream_info(
        (!parts[2].trim().is_empty()).then_some(parts[2].trim()),
        push_track,
    );

    Some(BranchItem {
        name: name.to_string(),
        display_name: None,
        is_head: head == "*",
        detached_head: false,
        upstream,
        recency: commit_timestamp
            .map(|timestamp| unix_to_time_ago(timestamp.0))
            .unwrap_or_default(),
        ahead_for_pull,
        behind_for_pull,
        ahead_for_push,
        behind_for_push,
        upstream_gone,
        upstream_remote,
        upstream_branch,
        subject: subject.to_string(),
        commit_hash: commit_hash.to_string(),
        commit_timestamp,
        behind_base_branch: 0,
    })
}

fn normalize_local_branch_name(name: &str) -> &str {
    name.strip_prefix("refs/heads/")
        .or_else(|| name.strip_prefix("heads/"))
        .unwrap_or(name)
}

fn parse_branch_commit_timestamp(raw: &str) -> Option<Timestamp> {
    raw.parse::<u64>().ok().map(Timestamp)
}

fn parse_upstream_remote(upstream: &str) -> Option<String> {
    let (remote, _) = upstream.split_once('/')?;
    (!remote.is_empty()).then(|| remote.to_string())
}

fn parse_upstream_branch(upstream: &str) -> Option<String> {
    let (_, branch) = upstream.split_once('/')?;
    (!branch.is_empty()).then(|| branch.to_string())
}

fn parse_upstream_info(upstream_name: Option<&str>, track: &str) -> (String, String, bool) {
    if upstream_name.is_none_or(str::is_empty) {
        return ("?".to_string(), "?".to_string(), false);
    }

    if track == "[gone]" {
        return ("?".to_string(), "?".to_string(), true);
    }

    (
        parse_track_difference(track, "ahead "),
        parse_track_difference(track, "behind "),
        false,
    )
}

fn parse_track_difference(track: &str, needle: &str) -> String {
    track
        .split(needle)
        .nth(1)
        .map(|suffix| {
            suffix
                .chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "0".to_string())
}

fn unix_to_time_ago(timestamp: u64) -> String {
    let now = unix_timestamp_now();
    format_seconds_ago(now.0.saturating_sub(timestamp))
}

fn format_seconds_ago(seconds_ago: u64) -> String {
    const PERIODS: [(&str, u64); 7] = [
        ("s", 1),
        ("m", 60),
        ("h", 60 * 60),
        ("d", 60 * 60 * 24),
        ("w", 60 * 60 * 24 * 7),
        ("M", (60 * 60 * 24 * 365) / 12),
        ("y", 60 * 60 * 24 * 365),
    ];

    for index in 1..PERIODS.len() {
        if seconds_ago < PERIODS[index].1 {
            return format!(
                "{}{}",
                seconds_ago / PERIODS[index - 1].1,
                PERIODS[index - 1].0
            );
        }
    }

    format!(
        "{}{}",
        seconds_ago / PERIODS[PERIODS.len() - 1].1,
        PERIODS[PERIODS.len() - 1].0
    )
}

fn read_branch_configs(repo_path: &Path) -> BTreeMap<String, BranchConfig> {
    let output = git_stdout_allow_failure(
        repo_path,
        [
            "config",
            "--local",
            "--get-regexp",
            r"^branch\..*\.(remote|merge)$",
        ],
    )
    .unwrap_or_default();
    parse_branch_configs(&output)
}

fn parse_branch_configs(output: &str) -> BTreeMap<String, BranchConfig> {
    let mut configs: BTreeMap<String, BranchConfig> = BTreeMap::new();

    for line in output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Some((key, value)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let value = value.trim();
        let Some(branch_name) = key.strip_prefix("branch.") else {
            continue;
        };
        let Some((branch_name, field)) = branch_name.rsplit_once('.') else {
            continue;
        };
        let config = configs.entry(branch_name.to_string()).or_default();
        match field {
            "remote" => config.remote = Some(value.to_string()),
            "merge" => config.merge = Some(value.to_string()),
            _ => {}
        }
    }

    configs
}

fn read_remote_urls_by_name(repo_path: &Path) -> BTreeMap<String, Vec<String>> {
    let output = git_stdout_allow_failure(
        repo_path,
        ["config", "--local", "--get-regexp", r"^remote\.[^.]+\.url$"],
    )
    .unwrap_or_default();
    parse_remote_urls_by_name(&output)
}

fn parse_remote_urls_by_name(output: &str) -> BTreeMap<String, Vec<String>> {
    let mut remotes = BTreeMap::<String, Vec<String>>::new();

    for line in output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Some((key, url)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let url = url.trim();
        if url.is_empty() {
            continue;
        }
        let Some(name) = key
            .strip_prefix("remote.")
            .and_then(|value| value.strip_suffix(".url"))
        else {
            continue;
        };
        remotes
            .entry(name.to_string())
            .or_default()
            .push(url.to_string());
    }

    remotes
}

fn read_remotes(repo_path: &Path, remote_branches: &[RemoteBranchItem]) -> Vec<RemoteItem> {
    let mut remotes = read_remote_urls_by_name(repo_path)
        .into_iter()
        .map(|(name, urls)| {
            let primary_url = urls.first().cloned().unwrap_or_default();
            (
                name.clone(),
                RemoteItem {
                    name,
                    fetch_url: primary_url.clone(),
                    push_url: primary_url,
                    branch_count: 0,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    if let Ok(output) = git_stdout(repo_path, ["remote", "-v"]) {
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
    }

    let mut remotes = remotes.into_values().collect::<Vec<_>>();
    for remote in &mut remotes {
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
    remotes.sort_by(compare_remote_items);

    remotes
}

fn compare_remote_items(left: &RemoteItem, right: &RemoteItem) -> Ordering {
    match (left.name == "origin", right.name == "origin") {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => {
            let lower_cmp = left.name.to_lowercase().cmp(&right.name.to_lowercase());
            if lower_cmp == Ordering::Equal {
                left.name.cmp(&right.name)
            } else {
                lower_cmp
            }
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct TagMetadata {
    target_oid: String,
    target_short_oid: String,
    annotated: bool,
}

fn parse_tag_listing(output: &str) -> Vec<(String, String)> {
    output.lines().filter_map(parse_tag_listing_line).collect()
}

fn parse_tag_listing_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let split_index = trimmed.find(char::is_whitespace);
    let (name, summary) = if let Some(index) = split_index {
        let summary = trimmed[index..].trim_start();
        (&trimmed[..index], summary)
    } else {
        (trimmed, "")
    };

    if name.is_empty() {
        return None;
    }

    Some((name.to_string(), summary.to_string()))
}

fn read_tag_metadata(repo_path: &Path) -> BTreeMap<String, TagMetadata> {
    git_stdout(
        repo_path,
        [
            "for-each-ref",
            "--format=%(refname:short)%00%(objecttype)%00%(objectname)%00%(*objectname)",
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
                Some((
                    name.to_string(),
                    TagMetadata {
                        target_oid: target_oid.to_string(),
                        target_short_oid: target_oid.chars().take(7).collect(),
                        annotated: object_type == "tag",
                    },
                ))
            })
            .collect()
    })
    .unwrap_or_default()
}

fn read_tags(repo_path: &Path) -> Vec<TagItem> {
    git_stdout(repo_path, ["tag", "--list", "-n", "--sort=-creatordate"])
        .map(|output| {
            let metadata = read_tag_metadata(repo_path);
            parse_tag_listing(&output)
                .into_iter()
                .filter_map(|(name, summary)| {
                    let metadata = metadata.get(&name)?;
                    Some(TagItem {
                        name,
                        target_oid: metadata.target_oid.clone(),
                        target_short_oid: metadata.target_short_oid.clone(),
                        summary,
                        annotated: metadata.annotated,
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

const COMMIT_HISTORY_PRETTY_FORMAT: &str = "%H%x00%at%x00%aN%x00%ae%x00%P%x00%m%x00%D%x00%s";

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedCommitLine {
    oid: String,
    short_oid: String,
    summary: String,
    tags: Vec<String>,
    extra_info: String,
    author_name: String,
    author_email: String,
    unix_timestamp: i64,
    parents: Vec<String>,
    divergence: CommitDivergence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PushedStatusRef {
    local_ref: String,
    upstream_ref: String,
}

fn read_commits(
    repo_path: &Path,
    commit_ref: Option<&str>,
    commit_history_mode: CommitHistoryMode,
) -> CommitHistoryResult {
    match commit_history_mode {
        CommitHistoryMode::Linear => read_linear_commits(repo_path, commit_ref),
        CommitHistoryMode::Graph { reverse } => read_graph_commits(repo_path, reverse),
        CommitHistoryMode::Reflog => read_reflog_commit_history(repo_path),
    }
}

fn read_linear_commits(repo_path: &Path, commit_ref: Option<&str>) -> CommitHistoryResult {
    let main_branch_refs = existing_main_branch_refs(repo_path).unwrap_or_default();
    let unmerged_hashes = reachable_hashes(
        repo_path,
        commit_ref.unwrap_or("HEAD"),
        main_branch_refs.as_slice(),
    );
    let unpushed_hashes = resolve_pushed_status_ref(repo_path, commit_ref).and_then(|status_ref| {
        let mut excluded_refs = vec![status_ref.upstream_ref];
        excluded_refs.extend(main_branch_refs.iter().cloned());
        reachable_hashes(
            repo_path,
            status_ref.local_ref.as_str(),
            excluded_refs.as_slice(),
        )
    });

    let mut args = vec![
        OsString::from("log"),
        OsString::from("--no-show-signature"),
        OsString::from(format!("--format={COMMIT_HISTORY_PRETTY_FORMAT}")),
        OsString::from("-n"),
        OsString::from("64"),
    ];
    if let Some(commit_ref) = commit_ref {
        args.push(OsString::from(commit_ref));
    }
    git_stdout(repo_path, args)
        .map(|output| {
            let mut commits = output
                .lines()
                .filter_map(|line| {
                    extract_commit_from_line(line, false)
                        .map(|parsed| hydrate_commit_item(repo_path, parsed))
                })
                .collect::<Vec<_>>();
            set_commit_statuses(
                unpushed_hashes.as_ref(),
                unmerged_hashes.as_ref(),
                &mut commits,
            );
            commits
        })
        .map(|commits| CommitHistoryResult {
            commits,
            graph_lines: Vec::new(),
        })
        .unwrap_or_default()
}

fn read_graph_commits(repo_path: &Path, reverse: bool) -> CommitHistoryResult {
    let main_branch_refs = existing_main_branch_refs(repo_path).unwrap_or_default();
    let unmerged_hashes = reachable_hashes(repo_path, "HEAD", main_branch_refs.as_slice());
    let unpushed_hashes = resolve_pushed_status_ref(repo_path, None).and_then(|status_ref| {
        let mut excluded_refs = vec![status_ref.upstream_ref];
        excluded_refs.extend(main_branch_refs.iter().cloned());
        reachable_hashes(
            repo_path,
            status_ref.local_ref.as_str(),
            excluded_refs.as_slice(),
        )
    });

    let args = vec![
        OsString::from("log"),
        OsString::from("--decorate=short"),
        OsString::from("--topo-order"),
        OsString::from("--no-show-signature"),
        OsString::from(format!("--format={COMMIT_HISTORY_PRETTY_FORMAT}")),
        OsString::from("--all"),
        OsString::from("-n"),
        OsString::from("64"),
    ];
    git_stdout(repo_path, args)
        .map(|output| {
            let mut commits = Vec::new();
            let mut graph_commits = Vec::new();
            let mut graph_suffixes = Vec::new();
            for line in output.lines() {
                let Some(parsed) = extract_commit_from_line(line, false) else {
                    continue;
                };
                let row = format_commit_graph_suffix(&parsed);
                graph_commits.push(GraphCommit {
                    oid: parsed.oid.clone(),
                    parents: parsed.parents.clone(),
                });
                graph_suffixes.push(row);
                commits.push(hydrate_commit_item(repo_path, parsed));
            }
            set_commit_statuses(
                unpushed_hashes.as_ref(),
                unmerged_hashes.as_ref(),
                &mut commits,
            );
            let graph_rows = render_commit_graph(&graph_commits);
            let mut graph_lines = if graph_rows.len() == graph_suffixes.len() {
                graph_rows
                    .into_iter()
                    .zip(graph_suffixes)
                    .map(|(graph_row, suffix)| format!("{graph_row} {suffix}"))
                    .collect::<Vec<_>>()
            } else {
                graph_suffixes
            };
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

fn extract_commit_from_line(line: &str, show_divergence: bool) -> Option<ParsedCommitLine> {
    let split = line.splitn(8, '\0').collect::<Vec<_>>();
    if split.len() < 7 {
        return None;
    }

    let oid = split[0].to_string();
    if oid.is_empty() {
        return None;
    }

    let author_name = split[2].to_string();
    let author_email = split[3].to_string();
    let parents = split[4]
        .split_whitespace()
        .filter(|parent| !parent.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let divergence = if show_divergence {
        match split[5] {
            "<" => CommitDivergence::Left,
            ">" => CommitDivergence::Right,
            _ => CommitDivergence::None,
        }
    } else {
        CommitDivergence::None
    };

    let raw_extra_info = split[6].trim();
    let tags = raw_extra_info
        .split(',')
        .map(str::trim)
        .filter_map(|field| field.strip_prefix("tag: ").map(str::to_string))
        .collect::<Vec<_>>();
    let extra_info = if raw_extra_info.is_empty() {
        String::new()
    } else {
        format!("({raw_extra_info})")
    };
    let summary = split.get(7).copied().unwrap_or_default().to_string();

    Some(ParsedCommitLine {
        short_oid: oid.chars().take(7).collect(),
        oid,
        summary,
        tags,
        extra_info,
        author_name,
        author_email,
        unix_timestamp: split[1].parse::<i64>().unwrap_or_default(),
        parents,
        divergence,
    })
}

fn hydrate_commit_item(repo_path: &Path, parsed: ParsedCommitLine) -> CommitItem {
    let changed_files = read_commit_files(repo_path, parsed.oid.as_str());
    let diff = read_commit_diff(repo_path, parsed.oid.as_str());
    CommitItem {
        oid: parsed.oid,
        short_oid: parsed.short_oid,
        summary: parsed.summary,
        tags: parsed.tags,
        extra_info: parsed.extra_info,
        author_name: parsed.author_name,
        author_email: parsed.author_email,
        unix_timestamp: parsed.unix_timestamp,
        parents: parsed.parents,
        status: CommitStatus::None,
        todo_action: CommitTodoAction::None,
        todo_action_flag: String::new(),
        divergence: parsed.divergence,
        filter_paths: Vec::new(),
        changed_files,
        diff,
    }
}

fn format_commit_graph_suffix(parsed: &ParsedCommitLine) -> String {
    match (parsed.extra_info.is_empty(), parsed.summary.is_empty()) {
        (true, true) => parsed.short_oid.clone(),
        (true, false) => format!("{} {}", parsed.short_oid, parsed.summary),
        (false, true) => format!("{} {}", parsed.short_oid, parsed.extra_info),
        (false, false) => format!(
            "{} {} {}",
            parsed.short_oid, parsed.extra_info, parsed.summary
        ),
    }
}

fn resolve_pushed_status_ref(
    repo_path: &Path,
    commit_ref: Option<&str>,
) -> Option<PushedStatusRef> {
    let local_ref = resolve_local_branch_ref(repo_path, commit_ref)?;
    let branch_name = normalize_local_branch_name(local_ref.as_str());
    let upstream = format!("{branch_name}@{{u}}");
    let upstream_ref = git_stdout_allow_failure(
        repo_path,
        ["rev-parse", "--symbolic-full-name", upstream.as_str()],
    )
    .ok()?;
    let upstream_ref = upstream_ref.trim().to_string();
    if upstream_ref.is_empty() {
        return None;
    }

    Some(PushedStatusRef {
        local_ref,
        upstream_ref,
    })
}

fn resolve_local_branch_ref(repo_path: &Path, commit_ref: Option<&str>) -> Option<String> {
    match commit_ref {
        Some(reference) if reference.starts_with("refs/heads/") => Some(reference.to_string()),
        Some(reference) => {
            git_stdout_allow_failure(repo_path, ["rev-parse", "--symbolic-full-name", reference])
                .ok()
                .map(|output| output.trim().to_string())
                .filter(|output| output.starts_with("refs/heads/"))
        }
        None => current_branch_name(repo_path)
            .ok()
            .map(|branch| format!("refs/heads/{branch}")),
    }
}

fn reachable_hashes(
    repo_path: &Path,
    reference: &str,
    excluded_refs: &[String],
) -> Option<HashSet<String>> {
    let mut args = vec![OsString::from("rev-list"), OsString::from(reference)];
    args.extend(
        excluded_refs
            .iter()
            .map(|excluded| OsString::from(format!("^{excluded}"))),
    );
    let output = git_stdout_allow_failure(repo_path, args).ok()?;
    Some(
        output
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

fn set_commit_statuses(
    unpushed_hashes: Option<&HashSet<String>>,
    unmerged_hashes: Option<&HashSet<String>>,
    commits: &mut [CommitItem],
) {
    for commit in commits {
        if commit.todo_action != CommitTodoAction::None {
            continue;
        }

        commit.status = if unmerged_hashes.is_none_or(|hashes| hashes.contains(&commit.oid)) {
            if unpushed_hashes.is_some_and(|hashes| hashes.contains(&commit.oid)) {
                CommitStatus::Unpushed
            } else {
                CommitStatus::Pushed
            }
        } else {
            CommitStatus::Merged
        };
    }
}

fn read_commit_files(repo_path: &Path, oid: &str) -> Vec<CommitFileItem> {
    git_stdout(
        repo_path,
        [
            "show",
            "--format=",
            "--submodule",
            "--no-ext-diff",
            "--name-status",
            "-z",
            "--no-renames",
            oid,
        ],
    )
    .map(|output| parse_name_status_entries(&output))
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

fn read_stashes(repo_path: &Path, filter_path: Option<&Path>) -> Vec<StashItem> {
    let Some(filter_path) = filter_path.filter(|path| !path.as_os_str().is_empty()) else {
        return read_unfiltered_stashes(repo_path);
    };

    read_filtered_stashes(repo_path, filter_path)
        .unwrap_or_else(|| read_unfiltered_stashes(repo_path))
}

fn read_unfiltered_stashes(repo_path: &Path) -> Vec<StashItem> {
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

fn read_filtered_stashes(repo_path: &Path, filter_path: &Path) -> Option<Vec<StashItem>> {
    let filter_path = normalize_git_path(filter_path);
    if filter_path.is_empty() {
        return Some(read_unfiltered_stashes(repo_path));
    }

    let output = git_stdout(
        repo_path,
        ["stash", "list", "--name-only", "--pretty=%gd:%H|%ct|%gs"],
    )
    .ok()?;
    parse_filtered_stashes_output(repo_path, &output, &filter_path)
}

fn parse_filtered_stashes_output(
    repo_path: &Path,
    output: &str,
    filter_path: &str,
) -> Option<Vec<StashItem>> {
    let lines = output.lines().collect::<Vec<_>>();
    let mut stashes = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        let Some(mut stash) = parse_filtered_stash_line(line) else {
            index += 1;
            continue;
        };

        index += 1;
        while index < lines.len() && !lines[index].starts_with("stash@{") {
            if lines[index].starts_with(filter_path) {
                stash.changed_files = read_stash_files(repo_path, &stash.stash_ref);
                stashes.push(stash);
                index += 1;
                break;
            }
            index += 1;
        }
    }

    Some(stashes)
}

fn parse_filtered_stash_line(line: &str) -> Option<StashItem> {
    let (stash_ref, metadata) = line.split_once(':')?;
    parse_stash_ref_index(stash_ref)?;
    let summary = metadata
        .split_once('|')
        .and_then(|(_, rest)| rest.split_once('|').map(|(_, message)| message))
        .unwrap_or(metadata);
    Some(StashItem {
        stash_ref: stash_ref.to_string(),
        label: format!("{stash_ref}: {summary}"),
        changed_files: Vec::new(),
    })
}

fn parse_stash_ref_index(stash_ref: &str) -> Option<usize> {
    stash_ref
        .strip_prefix("stash@{")?
        .strip_suffix('}')?
        .parse()
        .ok()
}

fn normalize_git_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn read_stash_files(repo_path: &Path, stash_ref: &str) -> Vec<CommitFileItem> {
    git_stdout(
        repo_path,
        [
            "stash",
            "show",
            "--submodule",
            "--no-ext-diff",
            "--name-status",
            "-z",
            "--no-renames",
            stash_ref,
        ],
    )
    .map(|output| parse_name_status_entries(&output))
    .unwrap_or_default()
}

fn parse_name_status_entries(output: &str) -> Vec<CommitFileItem> {
    let entries = output
        .trim_end_matches('\0')
        .split('\0')
        .filter(|entry| !entry.is_empty())
        .collect::<Vec<_>>();

    entries
        .chunks_exact(2)
        .map(|chunk| CommitFileItem {
            path: PathBuf::from(chunk[1]),
            kind: commit_status_kind(chunk[0]),
        })
        .collect()
}

fn read_reflog_commit_history(repo_path: &Path) -> CommitHistoryResult {
    match read_reflog_commits(repo_path, None, None, None) {
        Ok((commits, _)) => CommitHistoryResult {
            commits,
            graph_lines: Vec::new(),
        },
        Err(_) => CommitHistoryResult::default(),
    }
}

fn read_reflog_commits(
    repo_path: &Path,
    last_reflog_commit: Option<&CommitItem>,
    filter_path: Option<&Path>,
    filter_author: Option<&str>,
) -> GitResult<(Vec<CommitItem>, bool)> {
    let output = git_stdout(
        repo_path,
        build_reflog_commit_command(filter_path, filter_author).into_args(),
    )?;
    Ok(parse_reflog_commits_output(
        repo_path,
        &output,
        last_reflog_commit,
        filter_path,
    ))
}

fn build_reflog_commit_command(
    filter_path: Option<&Path>,
    filter_author: Option<&str>,
) -> GitCommandBuilder {
    let mut builder = GitCommandBuilder::new("log")
        .config("log.showSignature=false")
        .arg(["-g", "--format=+%H%x00%ct%x00%gs%x00%P"]);
    if let Some(filter_author) = filter_author.filter(|author| !author.is_empty()) {
        builder = builder.arg([OsString::from(format!("--author={filter_author}"))]);
    }
    if let Some(filter_path) = filter_path.filter(|path| !path.as_os_str().is_empty()) {
        builder = builder.arg(vec![
            OsString::from("--follow"),
            OsString::from("--name-status"),
            OsString::from("--"),
            filter_path.as_os_str().to_os_string(),
        ]);
    }
    builder
}

fn parse_reflog_commits_output(
    repo_path: &Path,
    output: &str,
    last_reflog_commit: Option<&CommitItem>,
    filter_path: Option<&Path>,
) -> (Vec<CommitItem>, bool) {
    let mut commits = Vec::new();
    let mut current_commit = None;
    let mut current_filter_paths = Vec::new();
    let mut only_obtained_new_reflog_commits = false;

    for line in output.lines() {
        if line.is_empty() {
            continue;
        }

        if let Some(line) = line.strip_prefix('+') {
            if let Some(commit) = finalize_reflog_commit(
                current_commit.take(),
                &mut current_filter_paths,
                filter_path,
            ) {
                commits.push(commit);
            }

            let Some(commit) = parse_reflog_commit_line(repo_path, line) else {
                continue;
            };
            if last_reflog_commit.is_some_and(|last| same_reflog_commit(&commit, last)) {
                only_obtained_new_reflog_commits = true;
                current_filter_paths.clear();
                current_commit = None;
                break;
            }
            current_commit = Some(commit);
            continue;
        }

        if current_commit.is_some() && filter_path.is_some() {
            let mut fields = line.split('\t');
            let _ = fields.next();
            current_filter_paths.extend(fields.map(PathBuf::from));
        }
    }

    if let Some(commit) =
        finalize_reflog_commit(current_commit, &mut current_filter_paths, filter_path)
    {
        commits.push(commit);
    }

    (commits, only_obtained_new_reflog_commits)
}

fn finalize_reflog_commit(
    commit: Option<CommitItem>,
    filter_paths: &mut Vec<PathBuf>,
    filter_path: Option<&Path>,
) -> Option<CommitItem> {
    let mut commit = commit?;
    if let Some(filter_path) = filter_path {
        if filter_paths
            .iter()
            .any(|path| !path.starts_with(filter_path))
        {
            commit.filter_paths = std::mem::take(filter_paths);
        } else {
            filter_paths.clear();
        }
    } else {
        filter_paths.clear();
    }
    Some(commit)
}

fn parse_reflog_commit_line(repo_path: &Path, line: &str) -> Option<CommitItem> {
    let fields = line.splitn(4, '\0').collect::<Vec<_>>();
    if fields.len() <= 3 {
        return None;
    }

    let oid = fields[0].to_string();
    if oid.is_empty() {
        return None;
    }

    let changed_files = read_commit_files(repo_path, oid.as_str());
    let diff = read_commit_diff(repo_path, oid.as_str());
    let parents = fields[3]
        .split_whitespace()
        .filter(|parent| !parent.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();

    Some(CommitItem {
        short_oid: oid.chars().take(7).collect(),
        oid,
        summary: fields[2].to_string(),
        tags: Vec::new(),
        extra_info: String::new(),
        author_name: String::new(),
        author_email: String::new(),
        unix_timestamp: fields[1].parse::<i64>().unwrap_or_default(),
        parents,
        status: CommitStatus::Reflog,
        todo_action: CommitTodoAction::None,
        todo_action_flag: String::new(),
        divergence: CommitDivergence::None,
        filter_paths: Vec::new(),
        changed_files,
        diff,
    })
}

fn same_reflog_commit(a: &CommitItem, b: &CommitItem) -> bool {
    a.oid == b.oid && a.unix_timestamp == b.unix_timestamp && a.summary == b.summary
}

fn read_reflog(repo_path: &Path) -> Vec<ReflogItem> {
    git_stdout(
        repo_path,
        [
            "reflog",
            "--format=%gD%x00%H%x00%h%x00%ct%x00%gs",
            "-n",
            "64",
        ],
    )
    .map(|output| {
        output
            .lines()
            .map(|line| {
                let mut parts = line.split('\0');
                let selector = parts.next().unwrap_or_default().to_string();
                let oid = parts.next().unwrap_or_default().to_string();
                let short_oid = parts.next().unwrap_or_default().to_string();
                let unix_timestamp = parts
                    .next()
                    .unwrap_or_default()
                    .parse::<i64>()
                    .unwrap_or_default();
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
                    unix_timestamp,
                    summary,
                    description,
                }
            })
            .collect()
    })
    .unwrap_or_default()
}

fn read_worktrees(repo_path: &Path) -> Vec<WorktreeItem> {
    let repo_paths = match RepoPaths::resolve(repo_path) {
        Ok(paths) => paths,
        Err(_) => return Vec::new(),
    };
    let output = match git_stdout(
        repo_paths.worktree_path(),
        ["worktree", "list", "--porcelain"],
    ) {
        Ok(output) => output,
        Err(_) => return Vec::new(),
    };

    let mut items = Vec::new();
    let mut current: Option<WorktreeItem> = None;

    for line in output.lines() {
        if line.is_empty() {
            push_worktree_item(&mut items, &mut current);
            continue;
        }

        if line == "bare" {
            current = None;
            continue;
        }

        if let Some(path) = line.strip_prefix("worktree ") {
            push_worktree_item(&mut items, &mut current);

            let raw_path = PathBuf::from(path);
            let is_path_missing = worktree_path_missing(&raw_path);
            let path = if is_path_missing {
                raw_path.clone()
            } else {
                canonicalize_existing_path(&raw_path)
            };
            current = Some(WorktreeItem {
                is_main: path == repo_paths.repo_path(),
                is_current: path == repo_paths.worktree_path(),
                path,
                is_path_missing,
                ..WorktreeItem::default()
            });
            continue;
        }

        let Some(item) = current.as_mut() else {
            continue;
        };

        if let Some(head) = line.strip_prefix("HEAD ") {
            item.head = head.to_string();
            continue;
        }

        if let Some(branch) = line.strip_prefix("branch ") {
            item.branch = Some(short_head_name(branch));
        }
    }
    push_worktree_item(&mut items, &mut current);

    for item in &mut items {
        if item.is_path_missing {
            continue;
        }
        item.git_dir = worktree_git_dir(&item.path);
    }

    let names = unique_worktree_names(
        &items
            .iter()
            .map(|item| item.path.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
    );
    for (item, name) in items.iter_mut().zip(names) {
        item.name = name;
    }

    if let Some(index) = items.iter().position(|item| item.is_current) {
        let current_item = items.remove(index);
        items.insert(0, current_item);
    }

    for item in &mut items {
        if item.branch.is_some() {
            continue;
        }
        let Some(git_dir) = item.git_dir.as_deref() else {
            continue;
        };
        if let Some(branch) = rebased_branch(git_dir) {
            item.branch = Some(branch);
            continue;
        }
        if let Some(branch) = bisected_branch(git_dir) {
            item.branch = Some(branch);
        }
    }

    items
}

fn push_worktree_item(items: &mut Vec<WorktreeItem>, current: &mut Option<WorktreeItem>) {
    if let Some(item) = current.take() {
        items.push(item);
    }
}

fn worktree_path_missing(path: &Path) -> bool {
    match fs::metadata(path) {
        Ok(_) => false,
        Err(error) => error.kind() == std::io::ErrorKind::NotFound,
    }
}

fn worktree_git_dir(worktree_path: &Path) -> Option<PathBuf> {
    git_stdout_allow_failure(
        worktree_path,
        ["rev-parse", "--path-format=absolute", "--absolute-git-dir"],
    )
    .ok()
    .filter(|value| !value.is_empty())
    .map(|value| canonicalize_existing_path(Path::new(&value)))
}

fn rebased_branch(git_dir: &Path) -> Option<String> {
    ["rebase-merge", "rebase-apply"]
        .into_iter()
        .find_map(|dir| {
            read_trimmed_file(&git_dir.join(dir).join("head-name"))
                .map(|value| short_head_name(&value))
        })
}

fn bisected_branch(git_dir: &Path) -> Option<String> {
    read_trimmed_file(&git_dir.join("BISECT_START"))
}

fn read_trimmed_file(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn short_head_name(value: &str) -> String {
    value.trim().trim_start_matches("refs/heads/").to_string()
}

#[derive(Debug, Clone)]
struct IndexedPath {
    path: String,
    index: usize,
}

#[derive(Debug, Clone)]
struct IndexedName {
    name: String,
    index: usize,
}

fn unique_worktree_names(paths: &[String]) -> Vec<String> {
    let indexed_paths = paths
        .iter()
        .enumerate()
        .map(|(index, path)| IndexedPath {
            path: path.clone(),
            index,
        })
        .collect::<Vec<_>>();
    let indexed_names = unique_worktree_names_at_depth(indexed_paths, 0);
    let mut names = vec![String::new(); paths.len()];
    for indexed_name in indexed_names {
        names[indexed_name.index] = indexed_name.name;
    }
    names
}

fn unique_worktree_names_at_depth(paths: Vec<IndexedPath>, depth: usize) -> Vec<IndexedName> {
    if paths.is_empty() {
        return Vec::new();
    }
    if paths.len() == 1 {
        let path = &paths[0];
        return vec![IndexedName {
            index: path.index,
            name: slice_at_depth(&path.path, depth),
        }];
    }

    let mut groups: BTreeMap<String, Vec<IndexedPath>> = BTreeMap::new();
    for path in paths {
        let key = value_at_depth(&path.path, depth);
        groups.entry(key).or_default().push(path);
    }

    let mut names = Vec::new();
    for group in groups.into_values() {
        if group.len() == 1 {
            let path = &group[0];
            names.push(IndexedName {
                index: path.index,
                name: slice_at_depth(&path.path, depth),
            });
        } else {
            names.extend(unique_worktree_names_at_depth(group, depth + 1));
        }
    }
    names
}

fn value_at_depth(path: &str, depth: usize) -> String {
    let segments = normalized_path_segments(path);
    if depth >= segments.len() {
        String::new()
    } else {
        segments[segments.len() - 1 - depth].clone()
    }
}

fn slice_at_depth(path: &str, depth: usize) -> String {
    let segments = normalized_path_segments(path);
    if depth >= segments.len() {
        String::new()
    } else {
        segments[segments.len() - 1 - depth..].join("/")
    }
}

fn normalized_path_segments(path: &str) -> Vec<String> {
    path.replace('\\', "/")
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(ToOwned::to_owned)
        .collect()
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

fn read_bisect_state(repo_path: &Path) -> Option<BisectState> {
    if !git_path_exists(repo_path, "BISECT_START") {
        return None;
    }

    let (bad_term, good_term) = resolve_git_path(repo_path, "BISECT_TERMS")
        .and_then(|path| fs::read_to_string(path).ok())
        .map(|contents| {
            let mut terms = contents
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(ToString::to_string);
            (
                terms.next().unwrap_or_else(|| "bad".to_string()),
                terms.next().unwrap_or_else(|| "good".to_string()),
            )
        })
        .unwrap_or_else(|| ("bad".to_string(), "good".to_string()));

    let current_commit = resolve_git_path(repo_path, "BISECT_EXPECTED_REV")
        .and_then(|path| read_trimmed_file(&path));
    let current_summary = current_commit
        .as_deref()
        .and_then(|commit| git_stdout(repo_path, ["show", "-s", "--format=%s", commit]).ok());

    Some(BisectState {
        bad_term,
        good_term,
        current_commit,
        current_summary,
    })
}

fn read_working_tree_state(repo_path: &Path) -> WorkingTreeState {
    WorkingTreeState {
        rebasing: is_rebase_in_progress(repo_path),
        merging: is_merge_in_progress(repo_path),
        cherry_picking: is_cherry_pick_in_progress(repo_path),
        reverting: is_revert_in_progress(repo_path),
    }
}

fn read_merge_state(working_tree_state: WorkingTreeState) -> MergeState {
    if working_tree_state.merging {
        MergeState::MergeInProgress
    } else if working_tree_state.rebasing {
        MergeState::RebaseInProgress
    } else if working_tree_state.cherry_picking {
        MergeState::CherryPickInProgress
    } else if working_tree_state.reverting {
        MergeState::RevertInProgress
    } else {
        MergeState::None
    }
}

fn is_rebase_in_progress(repo_path: &Path) -> bool {
    git_path_exists(repo_path, "rebase-merge") || git_path_exists(repo_path, "rebase-apply")
}

fn is_merge_in_progress(repo_path: &Path) -> bool {
    git_path_exists(repo_path, "MERGE_HEAD")
}

fn is_cherry_pick_in_progress(repo_path: &Path) -> bool {
    let Some(cherry_pick_head) =
        resolve_git_path(repo_path, "CHERRY_PICK_HEAD").and_then(|path| read_trimmed_file(&path))
    else {
        return false;
    };

    let stopped_sha = resolve_git_path(repo_path, "rebase-merge/stopped-sha")
        .and_then(|path| read_trimmed_file(&path));
    if stopped_sha
        .as_deref()
        .is_some_and(|stopped_sha| cherry_pick_head.starts_with(stopped_sha))
    {
        return false;
    }

    true
}

fn is_revert_in_progress(repo_path: &Path) -> bool {
    git_path_exists(repo_path, "REVERT_HEAD")
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

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParsedPatchLineKind {
    Header,
    HunkHeader,
    Addition,
    Deletion,
    Context,
    NoNewlineMarker,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedPatchLine {
    content: String,
    kind: ParsedPatchLineKind,
}

#[allow(dead_code)]
impl ParsedPatchLine {
    fn is_change(&self) -> bool {
        matches!(
            self.kind,
            ParsedPatchLineKind::Addition | ParsedPatchLineKind::Deletion
        )
    }
}

#[allow(dead_code)]
impl ParsedPatch {
    fn format_plain(&self) -> String {
        let mut lines = self.header_lines.clone();
        for hunk in &self.hunks {
            lines.extend(hunk.raw.lines().map(ToString::to_string));
        }
        lines.join("\n")
    }

    fn format_range_plain(&self, start_idx: usize, end_idx: usize) -> String {
        let lines = self.lines();
        if lines.is_empty() || start_idx > end_idx {
            return String::new();
        }

        let start = start_idx.min(lines.len().saturating_sub(1));
        let end = end_idx.min(lines.len().saturating_sub(1));
        lines[start..=end]
            .iter()
            .map(|line| line.content.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn lines(&self) -> Vec<ParsedPatchLine> {
        let mut lines = Vec::new();
        lines.extend(
            self.header_lines
                .iter()
                .cloned()
                .map(|content| ParsedPatchLine {
                    content,
                    kind: ParsedPatchLineKind::Header,
                }),
        );
        for hunk in &self.hunks {
            lines.extend(hunk.all_lines());
        }
        lines
    }

    fn hunk_old_start_for_line(&self, idx: usize) -> u32 {
        self.hunk_containing_line(idx)
            .map(|hunk_idx| self.hunks[hunk_idx].selection.old_start)
            .unwrap_or(0)
    }

    fn hunk_start_idx(&self, hunk_index: usize) -> usize {
        if self.hunks.is_empty() {
            return 0;
        }

        let hunk_index = hunk_index.min(self.hunks.len() - 1);
        self.header_lines.len()
            + self
                .hunks
                .iter()
                .take(hunk_index)
                .map(ParsedHunk::line_count)
                .sum::<usize>()
    }

    fn hunk_end_idx(&self, hunk_index: usize) -> usize {
        if self.hunks.is_empty() {
            return 0;
        }

        let hunk_index = hunk_index.min(self.hunks.len() - 1);
        self.hunk_start_idx(hunk_index) + self.hunks[hunk_index].line_count() - 1
    }

    fn contains_changes(&self) -> bool {
        self.hunks.iter().any(ParsedHunk::contains_changes)
    }

    fn line_number_of_line(&self, idx: usize) -> u32 {
        if idx < self.header_lines.len() || self.hunks.is_empty() {
            return 1;
        }

        let Some(hunk_idx) = self.hunk_containing_line(idx) else {
            let last_hunk = self.hunks.last().expect("non-empty hunks");
            return last_hunk.selection.new_start + last_hunk.selection.new_lines.saturating_sub(1);
        };

        let hunk = &self.hunks[hunk_idx];
        let hunk_start_idx = self.hunk_start_idx(hunk_idx);
        let idx_in_hunk = idx - hunk_start_idx;
        if idx_in_hunk == 0 {
            return hunk.selection.new_start;
        }

        let offset = hunk.body_lines()[..idx_in_hunk - 1]
            .iter()
            .filter(|line| {
                matches!(
                    line.kind,
                    ParsedPatchLineKind::Addition | ParsedPatchLineKind::Context
                )
            })
            .count() as u32;
        hunk.selection.new_start + offset
    }

    fn hunk_containing_line(&self, idx: usize) -> Option<usize> {
        self.hunks.iter().enumerate().find_map(|(hunk_idx, hunk)| {
            let start = self.hunk_start_idx(hunk_idx);
            let end = start + hunk.line_count();
            (idx >= start && idx < end).then_some(hunk_idx)
        })
    }

    fn get_next_change_idx_of_same_included_state(
        &self,
        idx: usize,
        included_lines: &[usize],
        included: bool,
    ) -> (usize, bool) {
        if self.line_count() == 0 {
            return (0, false);
        }

        let idx = idx.min(self.line_count() - 1);
        let lines = self.lines();
        let is_match = |line_idx: usize, line: &ParsedPatchLine| {
            let same_included_state = included_lines.contains(&line_idx) == included;
            line.is_change() && same_included_state
        };

        for (offset, line) in lines[idx..].iter().enumerate() {
            if is_match(idx + offset, line) {
                return (idx + offset, true);
            }
        }

        for line_idx in (0..lines.len()).rev() {
            if is_match(line_idx, &lines[line_idx]) {
                return (line_idx, true);
            }
        }

        (0, false)
    }

    fn get_next_change_idx(&self, idx: usize) -> usize {
        self.get_next_change_idx_of_same_included_state(idx, &[], false)
            .0
    }

    fn line_count(&self) -> usize {
        self.header_lines.len() + self.hunks.iter().map(ParsedHunk::line_count).sum::<usize>()
    }

    fn hunk_count(&self) -> usize {
        self.hunks.len()
    }

    fn adjust_line_number(&self, line_number: u32) -> u32 {
        let mut adjusted = line_number;
        for hunk in &self.hunks {
            if hunk.selection.old_start >= line_number {
                break;
            }
            if hunk.selection.old_start + hunk.selection.old_lines > line_number {
                return hunk.selection.new_start;
            }
            adjusted = adjusted + hunk.selection.new_lines - hunk.selection.old_lines;
        }
        adjusted
    }

    fn is_single_hunk_for_whole_file(&self) -> bool {
        if self.hunks.len() != 1 {
            return false;
        }

        let body_lines = self.hunks[0].body_lines();
        !body_lines.iter().any(|line| {
            matches!(
                line.kind,
                ParsedPatchLineKind::Context | ParsedPatchLineKind::Deletion
            )
        }) || !body_lines.iter().any(|line| {
            matches!(
                line.kind,
                ParsedPatchLineKind::Context | ParsedPatchLineKind::Addition
            )
        })
    }
}

#[allow(dead_code)]
impl ParsedHunk {
    fn body_lines(&self) -> Vec<ParsedPatchLine> {
        self.raw
            .lines()
            .skip(1)
            .map(|content| ParsedPatchLine {
                content: content.to_string(),
                kind: match content.chars().next() {
                    Some('+') => ParsedPatchLineKind::Addition,
                    Some('-') => ParsedPatchLineKind::Deletion,
                    Some(' ') => ParsedPatchLineKind::Context,
                    Some('\\') => ParsedPatchLineKind::NoNewlineMarker,
                    _ => ParsedPatchLineKind::Context,
                },
            })
            .collect()
    }

    fn all_lines(&self) -> Vec<ParsedPatchLine> {
        let mut lines = vec![ParsedPatchLine {
            content: self.raw.lines().next().unwrap_or_default().to_string(),
            kind: ParsedPatchLineKind::HunkHeader,
        }];
        lines.extend(self.body_lines());
        lines
    }

    fn line_count(&self) -> usize {
        self.raw.lines().count()
    }

    fn contains_changes(&self) -> bool {
        self.body_lines().iter().any(ParsedPatchLine::is_change)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super_lazygit_core::{
        CommitDivergence, CommitStatus, CommitTodoAction, DiffModel, GitCommand, GitCommandRequest,
        JobId, RebaseKind, RebaseStartMode, RepoId, ResetMode,
    };
    use super_lazygit_test_support::{
        clean_repo, conflicted_repo, detached_head_repo, dirty_repo, history_preview_repo,
        rebase_in_progress_repo, staged_and_unstaged_repo, stashed_repo, submodule_repo, temp_repo,
        upstream_diverged_repo, worktree_repo, TempRepo,
    };

    #[test]
    fn parse_custom_command_args_matches_basic_argv_rules() {
        assert_eq!(
            parse_custom_command_args(r#"printf "" "hello world" plain\ value"#)
                .expect("argv parsing"),
            vec![
                "printf".to_string(),
                String::new(),
                "hello world".to_string(),
                "plain value".to_string(),
            ]
        );
    }

    #[test]
    fn parse_custom_command_args_rejects_unterminated_quote() {
        let error = parse_custom_command_args(r#"printf "unterminated"#)
            .expect_err("unterminated quote should fail");
        assert_eq!(
            error,
            GitError::OperationFailed {
                message: "unterminated quote in custom command".to_string(),
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn custom_commands_run_with_output_captures_stdout_without_shell_expansion() {
        let repo = temp_repo().expect("fixture repo");
        let commands = CustomCommands::new(repo.path());

        assert_eq!(
            commands
                .run_with_output(r#"printf "$HOME""#)
                .expect("command output"),
            "$HOME"
        );
    }

    #[cfg(unix)]
    #[test]
    fn custom_commands_template_function_trims_trailing_crlf() {
        let repo = temp_repo().expect("fixture repo");
        let commands = CustomCommands::new(repo.path());

        assert_eq!(
            commands
                .template_function_run_command(r#"sh -c 'printf "hello\r\n"'"#)
                .expect("single line output"),
            "hello"
        );
    }

    #[cfg(unix)]
    #[test]
    fn custom_commands_template_function_rejects_crlf_multiline_output() {
        let repo = temp_repo().expect("fixture repo");
        let commands = CustomCommands::new(repo.path());

        let error = commands
            .template_function_run_command(r#"sh -c 'printf "hello\r\nworld\r\n"'"#)
            .expect_err("multiline output should fail");
        assert_eq!(
            error,
            GitError::OperationFailed {
                message: "command output contains newlines: hello\r\nworld".to_string(),
            }
        );
    }

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

        fn read_branch_merge_status(&self, _request: BranchMergeStatusRequest) -> GitResult<bool> {
            Ok(true)
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

    fn argv_strings(args: Vec<OsString>) -> Vec<String> {
        args.into_iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
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
    fn unique_worktree_names_match_upstream_cases() {
        let cases = [
            (Vec::<String>::new(), Vec::<&str>::new()),
            (vec!["/my/path/feature/one".to_string()], vec!["one"]),
            (vec!["/my/path/feature/one/".to_string()], vec!["one"]),
            (
                vec![
                    "/a/b/c/d".to_string(),
                    "/a/b/c/e".to_string(),
                    "/a/b/f/d".to_string(),
                    "/a/e/c/d".to_string(),
                ],
                vec!["b/c/d", "e", "f/d", "e/c/d"],
            ),
        ];

        for (paths, expected) in cases {
            assert_eq!(
                unique_worktree_names(&paths),
                expected.into_iter().map(str::to_string).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn build_push_command_matches_sync_go_argv_and_origin_guard() {
        let builder = build_push_command(&SyncPushOptions {
            force_with_lease: true,
            current_branch: "master".to_string(),
            upstream_remote: "origin".to_string(),
            upstream_branch: "main".to_string(),
            set_upstream: true,
            ..SyncPushOptions::default()
        })
        .expect("push command should build");
        assert_eq!(
            builder.to_argv(),
            vec![
                OsString::from("git"),
                OsString::from("push"),
                OsString::from("--force-with-lease"),
                OsString::from("--set-upstream"),
                OsString::from("origin"),
                OsString::from("refs/heads/master:main"),
            ]
        );

        let error = build_push_command(&SyncPushOptions {
            current_branch: "master".to_string(),
            upstream_branch: "main".to_string(),
            ..SyncPushOptions::default()
        })
        .expect_err("origin should be required for explicit upstream branch");
        assert_eq!(
            error,
            GitError::OperationFailed {
                message: "must specify origin when pushing to an explicit upstream branch"
                    .to_string(),
            }
        );
    }

    #[test]
    fn fetch_command_builder_matches_sync_go_argv() {
        assert_eq!(
            fetch_command_builder(false).to_argv(),
            vec![
                OsString::from("git"),
                OsString::from("fetch"),
                OsString::from("--no-write-fetch-head"),
            ]
        );
        assert_eq!(
            fetch_command_builder(true).to_argv(),
            vec![
                OsString::from("git"),
                OsString::from("fetch"),
                OsString::from("--all"),
                OsString::from("--no-write-fetch-head"),
            ]
        );
    }

    #[test]
    fn build_pull_command_matches_sync_go_argv() {
        let builder = build_pull_command(&SyncPullOptions {
            remote_name: "origin".to_string(),
            branch_name: "main".to_string(),
            fast_forward_only: true,
            worktree_git_dir: "/tmp/worktree/.git".to_string(),
            worktree_path: "/tmp/worktree".to_string(),
        });
        assert_eq!(
            builder.to_argv(),
            vec![
                OsString::from("git"),
                OsString::from("--git-dir"),
                OsString::from("/tmp/worktree/.git"),
                OsString::from("--work-tree"),
                OsString::from("/tmp/worktree"),
                OsString::from("pull"),
                OsString::from("--no-edit"),
                OsString::from("--ff-only"),
                OsString::from("origin"),
                OsString::from("refs/heads/main"),
            ]
        );
    }

    #[test]
    fn git_command_builder_matches_upstream_argv_ordering() {
        let scenarios = vec![
            (
                GitCommandBuilder::new("push")
                    .arg(["--force-with-lease"])
                    .arg(["--set-upstream"])
                    .arg(["origin"])
                    .arg(["master"])
                    .to_argv(),
                vec![
                    "git",
                    "push",
                    "--force-with-lease",
                    "--set-upstream",
                    "origin",
                    "master",
                ],
            ),
            (
                GitCommandBuilder::new("push")
                    .arg_if(true, ["--test"])
                    .to_argv(),
                vec!["git", "push", "--test"],
            ),
            (
                GitCommandBuilder::new("push")
                    .arg_if(false, ["--test"])
                    .to_argv(),
                vec!["git", "push"],
            ),
            (
                GitCommandBuilder::new("push")
                    .arg_if_else(true, "-b", "-a")
                    .to_argv(),
                vec!["git", "push", "-b"],
            ),
            (
                GitCommandBuilder::new("push")
                    .arg_if_else(false, "-a", "-b")
                    .to_argv(),
                vec!["git", "push", "-b"],
            ),
            (
                GitCommandBuilder::new("push").arg(["-a", "-b"]).to_argv(),
                vec!["git", "push", "-a", "-b"],
            ),
            (
                GitCommandBuilder::new("push")
                    .config("user.name=foo")
                    .config("user.email=bar")
                    .to_argv(),
                vec!["git", "-c", "user.email=bar", "-c", "user.name=foo", "push"],
            ),
            (
                GitCommandBuilder::new("push").dir("a/b/c").to_argv(),
                vec!["git", "-C", "a/b/c", "push"],
            ),
        ];

        for (input, expected) in scenarios {
            let expected: Vec<String> = expected.into_iter().map(str::to_owned).collect();
            assert_eq!(argv_strings(input), expected);
        }

        assert_eq!(
            GitCommandBuilder::new("push").dir("a/b/c").to_string(),
            "git -C a/b/c push"
        );
    }

    #[test]
    fn run_git_cmd_on_paths_matches_upstream_batching() {
        let long_path = |ch: &str| ch.repeat(9_000);
        let p1 = long_path("a");
        let p2 = long_path("b");
        let p3 = long_path("c");
        let p4 = long_path("d");

        let mut runs = Vec::new();
        run_git_cmd_on_paths_with_runner("checkout", Vec::<String>::new(), |builder| {
            runs.push(argv_strings(builder.to_argv()));
            Ok(())
        })
        .expect("empty path list succeeds");
        assert!(runs.is_empty());

        let mut runs = Vec::new();
        run_git_cmd_on_paths_with_runner(
            "checkout",
            vec![p1.clone(), p2.clone(), p3.clone()],
            |builder| {
                runs.push(argv_strings(builder.to_argv()));
                Ok(())
            },
        )
        .expect("single batch succeeds");
        assert_eq!(
            runs,
            vec![vec![
                "git".to_string(),
                "checkout".to_string(),
                "--".to_string(),
                p1.clone(),
                p2.clone(),
                p3.clone(),
            ]],
        );

        let mut runs = Vec::new();
        run_git_cmd_on_paths_with_runner(
            "checkout",
            vec![p1.clone(), p2.clone(), p3.clone(), p4.clone()],
            |builder| {
                runs.push(argv_strings(builder.to_argv()));
                Ok(())
            },
        )
        .expect("split batch succeeds");
        assert_eq!(
            runs,
            vec![
                vec![
                    "git".to_string(),
                    "checkout".to_string(),
                    "--".to_string(),
                    p1,
                    p2,
                    p3,
                ],
                vec![
                    "git".to_string(),
                    "checkout".to_string(),
                    "--".to_string(),
                    p4,
                ],
            ],
        );
    }

    fn reflog_loader_repo(
    ) -> std::io::Result<(super_lazygit_test_support::TempRepo, String, String)> {
        let repo = temp_repo()?;
        repo.write_file("reflog.txt", "base\n")?;
        repo.commit_all("initial")?;
        let parent = repo.rev_parse("HEAD")?;
        repo.write_file("reflog.txt", "base\nnext\n")?;
        repo.commit_all("next")?;
        let head = repo.rev_parse("HEAD")?;
        Ok((repo, head, parent))
    }

    #[test]
    fn build_reflog_commit_command_matches_upstream_argv() {
        let argv =
            build_reflog_commit_command(Some(Path::new("path")), Some("John Doe <john@doe.com>"))
                .to_argv();

        assert_eq!(
            argv_strings(argv),
            vec![
                "git".to_string(),
                "-c".to_string(),
                "log.showSignature=false".to_string(),
                "log".to_string(),
                "-g".to_string(),
                "--format=+%H%x00%ct%x00%gs%x00%P".to_string(),
                "--author=John Doe <john@doe.com>".to_string(),
                "--follow".to_string(),
                "--name-status".to_string(),
                "--".to_string(),
                "path".to_string(),
            ],
        );
    }

    #[test]
    fn parse_reflog_commits_output_matches_upstream_stop_logic() {
        let (repo, head, parent) = reflog_loader_repo().expect("fixture repo");
        let output = concat!(
            "+{head}\0{ts_new}\0checkout: moving from A to B\0{parent}\n",
            "+{head}\0{ts_new}\0checkout: moving from B to A\0{parent}\n",
            "+{head}\0{ts_new}\0checkout: moving from A to B\0{parent}\n",
            "+{parent}\0{ts_old}\0checkout: moving from A to master\0\n",
        )
        .replace("{head}", &head)
        .replace("{parent}", &parent)
        .replace("{ts_new}", "1643150483")
        .replace("{ts_old}", "1643149435");

        let (commits, only_new) = parse_reflog_commits_output(repo.path(), &output, None, None);
        assert!(!only_new);
        assert_eq!(commits.len(), 4);
        assert_eq!(commits[0].oid, head);
        assert_eq!(commits[0].summary, "checkout: moving from A to B");
        assert_eq!(commits[0].status, CommitStatus::Reflog);
        assert_eq!(commits[0].parents, vec![parent.clone()]);
        assert_eq!(commits[1].summary, "checkout: moving from B to A");
        assert_eq!(commits[3].oid, parent);
    }

    #[test]
    fn parse_reflog_commits_output_returns_only_new_entries() {
        let (repo, head, parent) = reflog_loader_repo().expect("fixture repo");
        let output = concat!(
            "+{head}\0{ts_new}\0checkout: moving from A to B\0{parent}\n",
            "+{head}\0{ts_new}\0checkout: moving from B to A\0{parent}\n",
            "+{parent}\0{ts_old}\0checkout: moving from A to master\0\n",
        )
        .replace("{head}", &head)
        .replace("{parent}", &parent)
        .replace("{ts_new}", "1643150483")
        .replace("{ts_old}", "1643149435");
        let last_reflog_commit = CommitItem {
            oid: head.clone(),
            summary: "checkout: moving from B to A".to_string(),
            unix_timestamp: 1_643_150_483,
            ..CommitItem::default()
        };

        let (commits, only_new) =
            parse_reflog_commits_output(repo.path(), &output, Some(&last_reflog_commit), None);
        assert!(only_new);
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].oid, head);
        assert_eq!(commits[0].summary, "checkout: moving from A to B");
    }

    #[test]
    fn parse_reflog_commits_output_tracks_filter_paths_for_renames() {
        let (repo, head, parent) = reflog_loader_repo().expect("fixture repo");
        let output = concat!(
            "+{head}\0{ts_new}\0checkout: moving from A to B\0{parent}\n",
            "R100\tpath/file.txt\tpath-renamed/file.txt\n",
            "+{parent}\0{ts_old}\0checkout: moving from A to master\0\n",
            "M\tpath/file.txt\n",
        )
        .replace("{head}", &head)
        .replace("{parent}", &parent)
        .replace("{ts_new}", "1643150483")
        .replace("{ts_old}", "1643149435");

        let (commits, only_new) = parse_reflog_commits_output(
            repo.path(),
            &output,
            None,
            Some(Path::new("path/file.txt")),
        );
        assert!(!only_new);
        assert_eq!(commits.len(), 2);
        assert_eq!(
            commits[0].filter_paths,
            vec![
                PathBuf::from("path/file.txt"),
                PathBuf::from("path-renamed/file.txt"),
            ],
        );
        assert!(commits[1].filter_paths.is_empty());
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
        assert_eq!(
            facade
                .route_for(GitOperationKind::ReadBranchMergeStatus)
                .backend,
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
    fn repo_paths_resolve_standard_repo_from_nested_path() {
        let repo = clean_repo().expect("fixture repo");
        repo.write_file("nested/file.txt", "nested\n")
            .expect("nested fixture file");
        let nested = repo.path().join("nested");
        let canonical_repo_path = fs::canonicalize(repo.path()).expect("canonical repo root");
        let canonical_repo_git_dir =
            fs::canonicalize(repo.path().join(".git")).expect("canonical git dir");

        let paths = RepoPaths::resolve(&nested).expect("repo paths resolve");

        assert_eq!(paths.worktree_path(), canonical_repo_path.as_path());
        assert_eq!(
            paths.worktree_git_dir_path(),
            canonical_repo_git_dir.as_path()
        );
        assert_eq!(paths.git_dir(), canonical_repo_git_dir.as_path());
        assert_eq!(paths.repo_path(), canonical_repo_path.as_path());
        assert_eq!(paths.repo_git_dir_path(), canonical_repo_git_dir.as_path());
        assert_eq!(
            paths.repo_name(),
            canonical_repo_path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("repo name")
        );
        assert!(!paths.is_bare_repo());
        verify_in_git_repo(&nested).expect("nested path verifies");
    }

    #[test]
    fn repo_paths_resolve_linked_worktree_git_dir() {
        let repo = worktree_repo().expect("fixture repo");
        let worktree_path = repo
            .worktree_list()
            .expect("worktree list")
            .lines()
            .find_map(|line| {
                let path = line.strip_prefix("worktree ")?;
                path.ends_with("feature-tree").then(|| PathBuf::from(path))
            })
            .expect("linked worktree path");
        let canonical_repo_path = fs::canonicalize(repo.path()).expect("canonical repo path");
        let canonical_repo_git_dir =
            fs::canonicalize(repo.path().join(".git")).expect("canonical repo git dir");
        let canonical_worktree_path =
            fs::canonicalize(&worktree_path).expect("canonical worktree path");
        let canonical_worktree_git_dir = fs::canonicalize(
            repo.path()
                .join(".git")
                .join("worktrees")
                .join("feature-tree"),
        )
        .expect("canonical linked git dir");

        let paths = RepoPaths::resolve(&worktree_path).expect("worktree repo paths resolve");

        assert_eq!(paths.worktree_path(), canonical_worktree_path.as_path());
        assert_eq!(
            paths.worktree_git_dir_path(),
            canonical_worktree_git_dir.as_path()
        );
        assert_eq!(paths.git_dir(), canonical_worktree_git_dir.as_path());
        assert_eq!(paths.repo_path(), canonical_repo_path.as_path());
        assert_eq!(paths.repo_git_dir_path(), canonical_repo_git_dir.as_path());
        assert_eq!(
            paths.repo_name(),
            canonical_repo_path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("repo name")
        );
        assert!(!paths.is_bare_repo());
        verify_in_git_repo(&worktree_path).expect("worktree path verifies");
    }

    #[test]
    fn repo_paths_resolve_submodule_paths() {
        let repo = submodule_repo().expect("fixture repo");
        let submodule_path = repo.path().join("vendor/child-module");
        let canonical_submodule_path =
            fs::canonicalize(&submodule_path).expect("canonical submodule");
        let canonical_submodule_git_dir = fs::canonicalize(
            repo.path()
                .join(".git")
                .join("modules")
                .join("vendor")
                .join("child-module"),
        )
        .expect("canonical submodule git dir");

        let paths = RepoPaths::resolve(&submodule_path).expect("submodule repo paths resolve");

        assert_eq!(paths.worktree_path(), canonical_submodule_path.as_path());
        assert_eq!(
            paths.worktree_git_dir_path(),
            canonical_submodule_git_dir.as_path()
        );
        assert_eq!(paths.git_dir(), canonical_submodule_git_dir.as_path());
        assert_eq!(paths.repo_path(), canonical_submodule_path.as_path());
        assert_eq!(
            paths.repo_git_dir_path(),
            canonical_submodule_git_dir.as_path()
        );
        assert_eq!(paths.repo_name(), "child-module");
        assert!(!paths.is_bare_repo());
        verify_in_git_repo(&submodule_path).expect("submodule path verifies");
    }

    #[test]
    fn repo_paths_resolve_bare_repo() {
        let root = tempfile::tempdir().expect("tempdir");
        let bare_parent = root.path().join("bare-repo");
        fs::create_dir_all(&bare_parent).expect("create bare repo parent");
        let bare_git_dir = bare_parent.join("bare.git");
        run_git(
            root.path(),
            &[
                "init",
                "--bare",
                bare_git_dir.to_str().unwrap_or("bare.git"),
            ],
        )
        .expect("init bare repo");
        let canonical_bare_git_dir =
            fs::canonicalize(&bare_git_dir).expect("canonical bare git dir");
        let canonical_bare_parent =
            fs::canonicalize(&bare_parent).expect("canonical bare repo parent");

        let paths = RepoPaths::resolve(&bare_git_dir).expect("bare repo paths resolve");

        assert_eq!(paths.worktree_path(), canonical_bare_git_dir.as_path());
        assert_eq!(
            paths.worktree_git_dir_path(),
            canonical_bare_git_dir.as_path()
        );
        assert_eq!(paths.git_dir(), canonical_bare_git_dir.as_path());
        assert_eq!(paths.repo_path(), canonical_bare_parent.as_path());
        assert_eq!(paths.repo_git_dir_path(), canonical_bare_git_dir.as_path());
        assert_eq!(paths.repo_name(), "bare-repo");
        assert!(paths.is_bare_repo());
        verify_in_git_repo(&bare_git_dir).expect("bare repo verifies");
    }

    #[test]
    fn verify_in_git_repo_rejects_non_repo_paths() {
        let dir = tempfile::tempdir().expect("tempdir");
        let canonical = fs::canonicalize(dir.path()).expect("canonical tempdir");

        let error = verify_in_git_repo(dir.path()).expect_err("non-repo should fail");

        assert!(matches!(error, GitError::OperationFailed { .. }));
        assert!(error.to_string().contains("not a git repository"));
        assert!(RepoPaths::resolve(&canonical).is_err());
    }

    #[test]
    fn git_helpers_set_optional_locks_env_and_preserve_extra_env() {
        let repo = clean_repo().expect("fixture repo");
        let output = git_output_with_env(
            repo.path(),
            [
                "-c",
                "alias.printenv=!printf '%s' \"$GIT_OPTIONAL_LOCKS:$GIT_EDITOR\"",
                "printenv",
            ],
            &[("GIT_EDITOR", OsStr::new("vim"))],
        )
        .expect("git alias runs");

        assert_eq!(stdout_string(output).expect("utf8 stdout"), "0:vim");
    }

    #[test]
    fn git_helpers_blame_line_range_returns_requested_lines() {
        let repo = blame_repo().expect("fixture repo");
        let second_commit = repo.rev_parse("HEAD").expect("second commit");

        let blamed = git_blame_line_range(repo.path(), Path::new("notes.txt"), "HEAD", 1, 2)
            .expect("blame range succeeds");

        assert!(blamed.contains(&second_commit));
        assert!(blamed.contains("alpha"));
        assert!(blamed.contains("beta updated"));
        assert!(!blamed.contains("gamma"));
    }

    #[test]
    fn git_helpers_retry_index_lock_failures_until_success() {
        let repo = clean_repo().expect("fixture repo");
        let (alias, attempt_file) =
            install_index_lock_retry_script(&repo, "git-retry-success.sh", Some(3));
        let output = git_output_with_env(
            repo.path(),
            ["-c".to_string(), alias, "retrylock".to_string()],
            &[],
        )
        .expect("git alias runs");

        assert!(output.status.success());
        assert_eq!(stdout_string(output).expect("utf8 stdout"), "3");
        assert_eq!(
            fs::read_to_string(attempt_file).expect("read attempt file"),
            "3"
        );
    }

    #[test]
    fn git_helpers_stop_retrying_after_index_lock_retry_budget() {
        let repo = clean_repo().expect("fixture repo");
        let (alias, attempt_file) =
            install_index_lock_retry_script(&repo, "git-retry-fail.sh", None);
        let output = git_output_with_env(
            repo.path(),
            ["-c".to_string(), alias, "retrylock".to_string()],
            &[],
        )
        .expect("git alias runs");

        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(GIT_INDEX_LOCK_MARKER));
        assert_eq!(
            fs::read_to_string(attempt_file).expect("read attempt file"),
            GIT_INDEX_LOCK_RETRY_COUNT.to_string()
        );
    }

    fn install_index_lock_retry_script(
        repo: &TempRepo,
        script_name: &str,
        succeed_after_attempt: Option<usize>,
    ) -> (String, PathBuf) {
        let script_path = repo.path().join(script_name);
        let attempt_file = repo
            .path()
            .join(".git")
            .join(format!("{script_name}.count"));
        let success_case = match succeed_after_attempt {
            Some(attempt) => format!(
                "if [ \"$count\" -lt {attempt} ]; then\n    printf '%s' '{GIT_INDEX_LOCK_MARKER}' >&2\n    exit 1\nfi\nprintf '%s' \"$count\"\n"
            ),
            None => format!("printf '%s' '{GIT_INDEX_LOCK_MARKER}' >&2\nexit 1\n"),
        };
        let script = format!(
            "#!/bin/sh\ncount_file='.git/{script_name}.count'\ncount=0\nif [ -f \"$count_file\" ]; then\n    count=$(cat \"$count_file\")\nfi\ncount=$((count + 1))\nprintf '%s' \"$count\" > \"$count_file\"\n{success_case}"
        );
        write_executable_script(&script_path, &script).expect("write retry script");
        (format!("alias.retrylock=!./{script_name}"), attempt_file)
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
        let parsed = parse_status("## main\0MM src/lib.rs\0");
        let entry = parsed.file_tree.first().expect("mixed entry");

        assert_eq!(parsed.staged_count, 1);
        assert_eq!(parsed.unstaged_count, 1);
        assert_eq!(entry.path, Path::new("src/lib.rs"));
        assert_eq!(entry.kind, FileStatusKind::Modified);
        assert_eq!(entry.staged_kind, Some(FileStatusKind::Modified));
        assert_eq!(entry.unstaged_kind, Some(FileStatusKind::Modified));
        assert_eq!(entry.short_status, "MM");
        assert_eq!(entry.display_string, "MM src/lib.rs");
    }

    #[test]
    fn parse_status_tracks_previous_path_for_renames() {
        let parsed = parse_status("## main\0RM new.txt\0old.txt\0");
        let entry = parsed.file_tree.first().expect("rename entry");

        assert_eq!(entry.path, Path::new("new.txt"));
        assert_eq!(entry.previous_path.as_deref(), Some(Path::new("old.txt")));
        assert_eq!(entry.kind, FileStatusKind::Renamed);
        assert_eq!(entry.short_status, "RM");
        assert_eq!(entry.display_string, "RM old.txt -> new.txt");
    }

    #[test]
    fn parse_numstat_tracks_renamed_destination_path() {
        let parsed = parse_numstat("1\t0\t\0old.txt\0new.txt\0");

        assert_eq!(parsed.get(Path::new("new.txt")), Some(&(1, 0)));
    }

    #[test]
    fn cli_backend_enriches_status_entries_for_renames() {
        let repo = temp_repo().expect("fixture repo");
        repo.write_file(
            "old.txt",
            "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\n",
        )
        .expect("write original");
        repo.commit_all("initial").expect("commit initial");
        repo.git(["mv", "old.txt", "new.txt"]).expect("rename");
        repo.write_file(
            "new.txt",
            "one\ntwo\nthree\nfour\nfive\nsix\nSEVEN\nEIGHT\nNINE\nTEN\n",
        )
        .expect("write renamed content");

        let detail = CliGitBackend
            .read_repo_detail(RepoDetailRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
                selected_path: None,
                diff_presentation: DiffPresentation::Staged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail succeeds");

        let renamed = detail
            .file_tree
            .iter()
            .find(|item| item.path == Path::new("new.txt"))
            .expect("renamed entry");
        assert_eq!(renamed.previous_path.as_deref(), Some(Path::new("old.txt")));
        assert_eq!(renamed.short_status, "RM");
        assert_eq!(renamed.lines_added, 4);
        assert_eq!(renamed.lines_deleted, 4);
    }

    #[test]
    fn cli_backend_marks_linked_worktree_status_entries() {
        let root = tempfile::tempdir().expect("tempdir");
        let repo_path = root.path().join("main");
        init_repo_at(&repo_path, "main.txt", "main\n", "initial").expect("init repo");
        run_git(&repo_path, &["branch", "feature"]).expect("create feature branch");
        run_git(&repo_path, &["worktree", "add", "linked", "feature"]).expect("add worktree");

        let detail = CliGitBackend
            .read_repo_detail(RepoDetailRequest {
                repo_id: RepoId::new(repo_path.display().to_string()),
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail succeeds");

        let worktree = detail
            .file_tree
            .iter()
            .find(|item| item.path == Path::new("linked"))
            .expect("worktree entry");
        assert!(worktree.is_worktree);
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
    fn cli_backend_filters_stashes_by_selected_path_prefix() {
        let repo = filtered_stash_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let src_detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id: repo_id.clone(),
                selected_path: Some(PathBuf::from("src/only-first.txt")),
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load for src filter");
        assert_eq!(src_detail.stashes.len(), 1);
        assert_eq!(src_detail.stashes[0].stash_ref, "stash@{1}");
        assert!(src_detail.stashes[0].label.ends_with("first path"));
        assert!(src_detail.stashes[0]
            .changed_files
            .iter()
            .any(|file| file.path == Path::new("src/only-first.txt")));

        let docs_detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id,
                selected_path: Some(PathBuf::from("docs")),
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load for docs filter");
        assert_eq!(docs_detail.stashes.len(), 1);
        assert_eq!(docs_detail.stashes[0].stash_ref, "stash@{0}");
        assert!(docs_detail.stashes[0].label.ends_with("second path"));
        assert!(docs_detail.stashes[0]
            .changed_files
            .iter()
            .any(|file| file.path == Path::new("docs/only-second.txt")));
    }

    #[test]
    fn parse_filtered_stash_line_rejects_invalid_stash_refs() {
        assert!(parse_filtered_stash_line("stash@{bad}:deadbeef|123|message").is_none());
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
    fn legacy_staged_stash_path_preserves_unstaged_changes() {
        let repo = staged_and_unstaged_repo().expect("fixture repo");

        stash_staged_changes_legacy(repo.path(), Some("legacy staged checkpoint"))
            .expect("legacy staged stash should succeed");

        assert!(repo
            .stash_list()
            .expect("stash list")
            .contains("legacy staged checkpoint"));
        assert_eq!(
            repo.status_porcelain().expect("status"),
            "M tracked.txt
?? untracked.txt"
        );
        let stashed_patch = git_stdout_raw(repo.path(), ["stash", "show", "-p", "stash@{0}"])
            .expect("stash show should succeed");
        assert!(stashed_patch.contains("staged.txt"));
        assert!(!stashed_patch.contains("tracked.txt"));
    }

    #[test]
    fn legacy_staged_stash_cleans_up_added_deleted_entries() {
        let repo = staged_untracked_with_unstaged_repo().expect("fixture repo");

        stash_staged_changes_legacy(repo.path(), Some("legacy staged untracked"))
            .expect("legacy staged stash should succeed");

        assert!(repo
            .stash_list()
            .expect("stash list")
            .contains("legacy staged untracked"));
        assert_eq!(repo.status_porcelain().expect("status"), "M tracked.txt");
        assert!(!repo.path().join("staged-untracked.txt").exists());
    }

    #[test]
    fn parse_git_version_accepts_apple_git_suffix() {
        assert_eq!(
            parse_git_version("git version 2.39.3 (Apple Git-146)"),
            Ok((2, 39, 3))
        );
    }

    #[test]
    fn parse_git_version_accepts_windows_git_suffix() {
        assert_eq!(
            parse_git_version("git version 2.44.0.windows.1"),
            Ok((2, 44, 0))
        );
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

        assert_eq!(
            detail.branches.first().map(|branch| branch.name.as_str()),
            Some("main")
        );
        assert!(detail.branches.iter().any(|branch| branch.name == "main"
            && branch.is_head
            && branch.upstream.as_deref() == Some("origin/main")
            && branch.upstream_remote.as_deref() == Some("origin")
            && branch.upstream_branch.as_deref() == Some("main")
            && branch.ahead_for_pull == "1"
            && branch.behind_for_pull == "1"
            && !branch.subject.is_empty()
            && !branch.commit_hash.is_empty()
            && branch.commit_timestamp.is_some()));
        assert!(detail
            .branches
            .iter()
            .all(|branch| !branch.name.starts_with("origin/")));
    }

    #[test]
    fn cli_backend_prepends_detached_head_to_branch_list() {
        let repo = detached_head_repo().expect("fixture repo");
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

        let current = detail.branches.first().expect("detached head branch row");
        assert!(current.is_head);
        assert!(current.detached_head);
        assert!(current.upstream.is_none());
        assert!(current.name.contains("HEAD"));
    }

    #[test]
    fn cli_backend_recovers_upstream_from_branch_config_without_local_remote_ref() {
        let repo = TempRepo::new().expect("fixture repo");
        repo.write_file("shared.txt", "base\n")
            .expect("write tracked file");
        repo.commit_all("initial").expect("initial commit");
        repo.git(["checkout", "-b", "topic"])
            .expect("create topic branch");
        repo.git(["config", "branch.topic.remote", "origin"])
            .expect("set branch remote");
        repo.git(["config", "branch.topic.merge", "refs/heads/topic"])
            .expect("set branch merge");
        repo.git(["checkout", "main"]).expect("checkout main");

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

        let topic = detail
            .branches
            .iter()
            .find(|branch| branch.name == "topic")
            .expect("topic branch should be listed");
        assert_eq!(topic.upstream.as_deref(), Some("origin/topic"));
        assert_eq!(topic.upstream_remote.as_deref(), Some("origin"));
        assert_eq!(topic.upstream_branch.as_deref(), Some("topic"));
        assert_eq!(topic.ahead_for_pull, "?");
        assert_eq!(topic.behind_for_pull, "?");
    }

    #[test]
    fn cli_backend_reads_merge_fast_forward_preference_from_git_config() {
        let repo = TempRepo::new().expect("fixture repo");
        repo.write_file("shared.txt", "base\n")
            .expect("write tracked file");
        repo.commit_all("initial").expect("initial commit");
        repo.git(["config", "merge.ff", "false"])
            .expect("set merge.ff");

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

        assert_eq!(
            detail.merge_fast_forward_preference,
            MergeFastForwardPreference::NoFastForward
        );
    }

    #[test]
    fn cli_backend_records_fast_forward_merge_targets_for_branches() {
        let repo = TempRepo::new().expect("fixture repo");
        repo.write_file("shared.txt", "base\n")
            .expect("write tracked file");
        repo.commit_all("initial").expect("initial commit");
        repo.checkout_new_branch("feature")
            .expect("create feature branch");
        repo.write_file("feature.txt", "feature work\n")
            .expect("write feature file");
        repo.commit_all("feature work")
            .expect("commit feature work");
        repo.checkout("main").expect("checkout main");

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

        assert_eq!(
            detail.fast_forward_merge_targets.get("feature"),
            Some(&true)
        );
    }

    #[test]
    fn parse_branch_line_trims_heads_prefix_and_marks_head() {
        let branch = parse_branch_line(
            concat!(
                "*\x00heads/feature\x00origin/feature\x00[ahead 2, behind 3]\x00[ahead 1]\x00",
                "ship it\x00deadbeef\x001700000000"
            ),
            &BTreeMap::new(),
        )
        .expect("branch line should parse");

        assert_eq!(
            branch,
            BranchItem {
                name: "feature".to_string(),
                display_name: None,
                is_head: true,
                detached_head: false,
                upstream: Some("origin/feature".to_string()),
                recency: unix_to_time_ago(1_700_000_000),
                ahead_for_pull: "2".to_string(),
                behind_for_pull: "3".to_string(),
                ahead_for_push: "1".to_string(),
                behind_for_push: "0".to_string(),
                upstream_gone: false,
                upstream_remote: Some("origin".to_string()),
                upstream_branch: Some("feature".to_string()),
                subject: "ship it".to_string(),
                commit_hash: "deadbeef".to_string(),
                commit_timestamp: Some(Timestamp(1_700_000_000)),
                behind_base_branch: 0,
            }
        );
    }

    #[test]
    fn parse_branch_line_handles_missing_upstream() {
        let branch = parse_branch_line(
            "\x00topic\x00\x00\x00\x00\x00feedface\x001700000001",
            &BTreeMap::new(),
        )
        .expect("branch line without upstream should parse");

        assert_eq!(
            branch,
            BranchItem {
                name: "topic".to_string(),
                is_head: false,
                upstream: None,
                detached_head: false,
                recency: unix_to_time_ago(1_700_000_001),
                ahead_for_pull: "?".to_string(),
                behind_for_pull: "?".to_string(),
                ahead_for_push: "?".to_string(),
                behind_for_push: "?".to_string(),
                commit_hash: "feedface".to_string(),
                commit_timestamp: Some(Timestamp(1_700_000_001)),
                ..BranchItem::default()
            }
        );
    }

    #[test]
    fn parse_branch_line_uses_branch_config_when_upstream_ref_is_missing() {
        let configs = BTreeMap::from([(
            "topic".to_string(),
            BranchConfig {
                remote: Some("origin".to_string()),
                merge: Some("refs/heads/topic".to_string()),
            },
        )]);
        let branch = parse_branch_line(
            "\x00topic\x00\x00\x00\x00\x00feedface\x001700000001",
            &configs,
        )
        .expect("branch line should parse");

        assert_eq!(branch.upstream.as_deref(), Some("origin/topic"));
        assert_eq!(branch.upstream_remote.as_deref(), Some("origin"));
        assert_eq!(branch.upstream_branch.as_deref(), Some("topic"));
        assert_eq!(branch.ahead_for_pull, "?");
        assert_eq!(branch.behind_for_pull, "?");
    }

    #[test]
    fn parse_branch_line_marks_gone_upstream() {
        let branch = parse_branch_line(
            "\x00topic\x00origin/topic\x00[gone]\x00\x00\x00feedface\x001700000001",
            &BTreeMap::new(),
        )
        .expect("branch line should parse");

        assert!(branch.upstream_gone);
        assert_eq!(branch.ahead_for_pull, "?");
        assert_eq!(branch.behind_for_pull, "?");
    }

    #[test]
    fn parse_branch_line_ignores_blank_and_malformed_rows() {
        assert!(parse_branch_line("", &BTreeMap::new()).is_none());
        assert!(parse_branch_line("warning: ignored row", &BTreeMap::new()).is_none());
    }

    #[test]
    fn parse_branch_configs_collects_remote_and_merge_pairs() {
        let configs = parse_branch_configs(
            "branch.main.remote origin\nbranch.main.merge refs/heads/main\nbranch.topic.remote upstream\n",
        );

        assert_eq!(
            configs.get("main"),
            Some(&BranchConfig {
                remote: Some("origin".to_string()),
                merge: Some("refs/heads/main".to_string()),
            })
        );
        assert_eq!(
            configs.get("topic"),
            Some(&BranchConfig {
                remote: Some("upstream".to_string()),
                merge: None,
            })
        );
    }

    #[test]
    fn branch_base_reference_prefers_existing_main_branch() {
        let repo = TempRepo::new().expect("fixture repo");
        repo.write_file("shared.txt", "base\n")
            .expect("write tracked file");
        repo.commit_all("initial").expect("initial commit");
        repo.git(["checkout", "-b", "feature"])
            .expect("create feature branch");
        repo.append_file("shared.txt", "feature\n")
            .expect("update feature branch");
        repo.commit_all("feature change").expect("feature commit");

        let branch = BranchItem {
            name: "feature".to_string(),
            ..BranchItem::default()
        };
        assert_eq!(
            branch_base_reference(repo.path(), &branch).expect("base branch should resolve"),
            Some("refs/heads/main".to_string())
        );
        assert_eq!(
            branch_behind_base_count(repo.path(), &branch, "refs/heads/main")
                .expect("behind count should resolve"),
            0
        );
    }

    #[test]
    fn parse_current_branch_list_entry_prefers_attached_symbolic_ref() {
        assert_eq!(
            parse_current_branch_list_entry("master", ""),
            "master".to_string()
        );
    }

    #[test]
    fn parse_current_branch_list_entry_prefers_detached_display_name() {
        let detached_output = concat!(
            "*\0",
            "679b0456\0",
            "（头指针在 679b0456 分离）\n",
            " \0",
            "679b0456\0",
            "refs/heads/master\n"
        );

        assert_eq!(
            parse_current_branch_list_entry("", detached_output),
            "（头指针在 679b0456 分离）".to_string()
        );
    }

    #[test]
    fn parse_current_branch_list_entry_falls_back_to_short_oid_for_detached_head() {
        let detached_output = concat!("*\0", "6f71c57a\0\n");

        assert_eq!(
            parse_current_branch_list_entry("", detached_output),
            "(HEAD detached at 6f71c57a)".to_string()
        );
    }

    #[test]
    fn parse_name_status_entries_preserves_whitespace_paths() {
        let parsed =
            parse_name_status_entries("M\0dir/space name.txt\0A\0tab\tname.rs\0D\0line\nname.md\0");

        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].kind, FileStatusKind::Modified);
        assert_eq!(parsed[0].path, PathBuf::from("dir/space name.txt"));
        assert_eq!(parsed[1].kind, FileStatusKind::Added);
        assert_eq!(parsed[1].path, PathBuf::from("tab\tname.rs"));
        assert_eq!(parsed[2].kind, FileStatusKind::Deleted);
        assert_eq!(parsed[2].path, PathBuf::from("line\nname.md"));
    }

    #[test]
    fn parse_name_status_entries_preserves_multi_file_order_and_status_kinds() {
        let parsed = parse_name_status_entries("MM\0Myfile\0M \0MyOtherFile\0 M\0YetAnother\0");

        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].path, PathBuf::from("Myfile"));
        assert_eq!(parsed[0].kind, FileStatusKind::Modified);
        assert_eq!(parsed[1].path, PathBuf::from("MyOtherFile"));
        assert_eq!(parsed[1].kind, FileStatusKind::Modified);
        assert_eq!(parsed[2].path, PathBuf::from("YetAnother"));
        assert_eq!(parsed[2].kind, FileStatusKind::Modified);
    }

    #[test]
    fn parse_name_status_entries_ignores_empty_and_incomplete_chunks() {
        assert!(parse_name_status_entries("").is_empty());
        assert!(parse_name_status_entries("\0").is_empty());

        let parsed = parse_name_status_entries("M\0tracked.txt\0A\0");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].path, PathBuf::from("tracked.txt"));
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
    fn extract_commit_from_line_parses_metadata_tags_and_divergence() {
        let parsed = extract_commit_from_line(
            "abc123456789\x001640000000\x00Jane Smith\x00jane@example.com\x00parent1 parent2\x00<\x00HEAD -> feature, tag: v1.0, tag: latest\x00ship it",
            true,
        )
        .expect("commit line should parse");

        assert_eq!(parsed.oid, "abc123456789");
        assert_eq!(parsed.short_oid, "abc1234");
        assert_eq!(parsed.summary, "ship it");
        assert_eq!(parsed.tags, vec!["v1.0".to_string(), "latest".to_string()]);
        assert_eq!(
            parsed.extra_info,
            "(HEAD -> feature, tag: v1.0, tag: latest)"
        );
        assert_eq!(parsed.author_name, "Jane Smith");
        assert_eq!(parsed.author_email, "jane@example.com");
        assert_eq!(parsed.unix_timestamp, 1_640_000_000);
        assert_eq!(
            parsed.parents,
            vec!["parent1".to_string(), "parent2".to_string()]
        );
        assert_eq!(parsed.divergence, CommitDivergence::Left);
    }

    #[test]
    fn extract_commit_from_line_accepts_missing_message_field() {
        let parsed = extract_commit_from_line(
            "abc123\x00timestamp\x00author\x00email\x00parent\x00>\x00tag: v1.0",
            true,
        )
        .expect("minimal commit line should parse");

        assert_eq!(parsed.summary, "");
        assert_eq!(parsed.tags, vec!["v1.0".to_string()]);
        assert_eq!(parsed.extra_info, "(tag: v1.0)");
        assert_eq!(parsed.divergence, CommitDivergence::Right);
        assert_eq!(parsed.unix_timestamp, 0);
    }

    #[test]
    fn set_commit_statuses_marks_unpushed_pushed_and_merged_commits() {
        let mut commits = vec![
            CommitItem {
                oid: "unpushed".to_string(),
                summary: "unpushed".to_string(),
                ..CommitItem::default()
            },
            CommitItem {
                oid: "todo".to_string(),
                summary: "todo".to_string(),
                todo_action: CommitTodoAction::UpdateRef,
                ..CommitItem::default()
            },
            CommitItem {
                oid: "pushed".to_string(),
                summary: "pushed".to_string(),
                ..CommitItem::default()
            },
            CommitItem {
                oid: "merged".to_string(),
                summary: "merged".to_string(),
                ..CommitItem::default()
            },
        ];
        let unpushed = HashSet::from(["unpushed".to_string()]);
        let unmerged = HashSet::from([
            "unpushed".to_string(),
            "pushed".to_string(),
            "todo".to_string(),
        ]);

        set_commit_statuses(Some(&unpushed), Some(&unmerged), &mut commits);

        assert_eq!(commits[0].status, CommitStatus::Unpushed);
        assert_eq!(commits[1].status, CommitStatus::None);
        assert_eq!(commits[2].status, CommitStatus::Pushed);
        assert_eq!(commits[3].status, CommitStatus::Merged);
    }

    #[test]
    fn cli_backend_reads_commit_files_with_whitespace_paths() {
        let repo = temp_repo().expect("fixture repo");
        repo.write_file("dir/space name.txt", "base\n")
            .expect("write spaced file");
        repo.git(["add", "dir/space name.txt"])
            .expect("stage spaced file");
        repo.git(["commit", "-m", "initial"])
            .expect("initial commit");

        repo.write_file("dir/space name.txt", "base\nchanged\n")
            .expect("update spaced file");
        repo.write_file("tab\tname.rs", "fn main() {}\n")
            .expect("write tab file");
        repo.git(["add", "dir/space name.txt", "tab\tname.rs"])
            .expect("stage whitespace paths");
        repo.git(["commit", "-m", "whitespace paths"])
            .expect("second commit");

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

        let latest = detail.commits.first().expect("latest commit");
        assert_eq!(latest.summary, "whitespace paths");
        assert_eq!(latest.changed_files.len(), 2);
        assert_eq!(
            latest.changed_files[0].path,
            PathBuf::from("dir/space name.txt")
        );
        assert_eq!(latest.changed_files[0].kind, FileStatusKind::Modified);
        assert_eq!(latest.changed_files[1].path, PathBuf::from("tab\tname.rs"));
        assert_eq!(latest.changed_files[1].kind, FileStatusKind::Added);
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
    fn cli_backend_reads_commit_metadata_and_statuses_from_linear_history() {
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

        let latest = detail.commits.first().expect("latest commit");
        assert_eq!(latest.summary, "local change");
        assert_eq!(latest.author_name, "Super Lazygit Tests");
        assert_eq!(latest.author_email, "tests@example.com");
        assert_eq!(latest.parents.len(), 1);
        assert_eq!(latest.status, CommitStatus::Unpushed);
        assert_eq!(latest.divergence, CommitDivergence::None);

        let initial = detail
            .commits
            .iter()
            .find(|commit| commit.summary == "initial")
            .expect("initial commit should be present");
        assert_eq!(initial.status, CommitStatus::Merged);
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
            .any(|line| line.contains('⏣') || line.contains('◯')));
        assert!(forward
            .commit_graph_lines
            .iter()
            .any(|line| line.contains('│') || line.contains('╮') || line.contains('╯')));
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
        let canonical_repo_path = fs::canonicalize(repo.path()).expect("canonical repo path");

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
        assert_eq!(detail.worktrees[0].path, canonical_repo_path);
        assert_eq!(detail.worktrees[0].branch.as_deref(), Some("main"));
        assert!(detail.worktrees[0].is_main);
        assert!(detail.worktrees[0].is_current);
        assert!(!detail.worktrees[0].is_path_missing);
        assert_eq!(
            detail.worktrees[0].name,
            repo.path().file_name().unwrap().to_string_lossy()
        );
        assert!(detail
            .worktrees
            .iter()
            .any(|item| item.branch.as_deref() == Some("feature")));
        assert!(detail.worktrees.iter().all(|item| !item.head.is_empty()));
        assert!(detail
            .worktrees
            .iter()
            .all(|item| item.is_path_missing || item.git_dir.is_some()));
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
    fn cli_backend_prioritizes_current_linked_worktree_and_recovers_rebase_branch() {
        let repo = worktree_repo().expect("fixture repo");
        let linked_worktree_path = repo
            .worktree_list()
            .expect("worktree list")
            .lines()
            .find_map(|line| {
                let path = line.strip_prefix("worktree ")?;
                path.ends_with("feature-tree").then(|| PathBuf::from(path))
            })
            .expect("linked worktree path");
        let canonical_linked_path =
            fs::canonicalize(&linked_worktree_path).expect("canonical linked worktree path");
        let linked_git_dir = RepoPaths::resolve(&linked_worktree_path)
            .expect("resolve linked worktree paths")
            .worktree_git_dir_path()
            .to_path_buf();
        run_git(&linked_worktree_path, &["checkout", "--detach"])
            .expect("detach linked worktree head");
        fs::create_dir_all(linked_git_dir.join("rebase-merge")).expect("create rebase dir");
        fs::write(
            linked_git_dir.join("rebase-merge").join("head-name"),
            "refs/heads/feature\n",
        )
        .expect("write rebase head-name");

        let backend = CliGitBackend;
        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id: RepoId::new(linked_worktree_path.display().to_string()),
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load from linked worktree");

        assert_eq!(detail.worktrees.len(), 2);
        assert_eq!(detail.worktrees[0].path, canonical_linked_path);
        assert!(detail.worktrees[0].is_current);
        assert!(!detail.worktrees[0].is_main);
        assert_eq!(detail.worktrees[0].branch.as_deref(), Some("feature"));
        assert_eq!(detail.worktrees[0].name, "feature-tree");
        assert!(detail.worktrees[0].git_dir.is_some());
        assert!(detail.worktrees[1].is_main);
        assert!(!detail.worktrees[1].is_current);
    }

    #[test]
    fn cli_backend_marks_missing_worktree_paths() {
        let repo = worktree_repo().expect("fixture repo");
        let linked_worktree_path = repo
            .worktree_list()
            .expect("worktree list")
            .lines()
            .find_map(|line| {
                let path = line.strip_prefix("worktree ")?;
                path.ends_with("feature-tree").then(|| PathBuf::from(path))
            })
            .expect("linked worktree path");
        fs::remove_dir_all(&linked_worktree_path).expect("remove linked worktree path");

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
            .expect("detail should load from main worktree");

        let missing = detail
            .worktrees
            .iter()
            .find(|item| item.path == linked_worktree_path)
            .expect("missing linked worktree listed");
        assert!(missing.is_path_missing);
        assert!(!missing.is_current);
        assert!(!missing.is_main);
        assert_eq!(missing.branch.as_deref(), Some("feature"));
        assert_eq!(missing.name, "feature-tree");
        assert!(missing.git_dir.is_none());
    }

    #[test]
    fn cli_backend_recovers_bisect_branch_from_linked_worktree_git_dir() {
        let repo = worktree_repo().expect("fixture repo");
        let linked_worktree_path = repo
            .worktree_list()
            .expect("worktree list")
            .lines()
            .find_map(|line| {
                let path = line.strip_prefix("worktree ")?;
                path.ends_with("feature-tree").then(|| PathBuf::from(path))
            })
            .expect("linked worktree path");
        let linked_git_dir = RepoPaths::resolve(&linked_worktree_path)
            .expect("resolve linked worktree paths")
            .worktree_git_dir_path()
            .to_path_buf();
        run_git(&linked_worktree_path, &["checkout", "--detach"])
            .expect("detach linked worktree head");
        fs::write(linked_git_dir.join("BISECT_START"), "feature\n").expect("write bisect start");

        let backend = CliGitBackend;
        let detail = backend
            .read_repo_detail(RepoDetailRequest {
                repo_id: RepoId::new(linked_worktree_path.display().to_string()),
                selected_path: None,
                diff_presentation: DiffPresentation::Unstaged,
                commit_ref: None,
                commit_history_mode: CommitHistoryMode::Linear,
                ignore_whitespace_in_diff: false,
                diff_context_lines: 3,
                rename_similarity_threshold: 50,
            })
            .expect("detail should load from linked worktree");

        assert_eq!(detail.worktrees[0].branch.as_deref(), Some("feature"));
        assert!(detail.worktrees[0].is_current);
        assert_eq!(detail.worktrees[0].name, "feature-tree");
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
        assert!(detail.working_tree_state.rebasing);
        assert!(!detail.working_tree_state.merging);
        assert!(!detail.working_tree_state.cherry_picking);
        assert!(!detail.working_tree_state.reverting);
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
    fn cli_backend_reads_bisect_in_progress_state() {
        let repo = history_preview_repo().expect("fixture repo");
        repo.git(["bisect", "start"]).expect("start bisect");
        repo.git(["bisect", "bad", "HEAD"]).expect("mark bad");
        repo.git(["bisect", "good", "HEAD~2"]).expect("mark good");
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

        assert_eq!(
            detail
                .bisect_state
                .as_ref()
                .map(|state| state.bad_term.as_str()),
            Some("bad")
        );
        assert_eq!(
            detail
                .bisect_state
                .as_ref()
                .map(|state| state.good_term.as_str()),
            Some("good")
        );
        assert_eq!(
            detail
                .bisect_state
                .as_ref()
                .and_then(|state| state.current_summary.as_deref()),
            Some("second")
        );
    }

    #[test]
    fn cli_backend_reads_bisect_started_before_any_terms_are_marked() {
        let repo = history_preview_repo().expect("fixture repo");
        repo.git(["bisect", "start"]).expect("start bisect");
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

        let bisect = detail.bisect_state.expect("bisect state is present");
        assert_eq!(bisect.bad_term, "bad");
        assert_eq!(bisect.good_term, "good");
        assert!(bisect.current_commit.is_none());
        assert!(bisect.current_summary.is_none());
    }

    #[test]
    fn cli_backend_reads_bisect_custom_terms_and_expected_revision() {
        let repo = history_preview_repo().expect("fixture repo");
        repo.git(["bisect", "start", "--term-old=old", "--term-new=new"])
            .expect("start bisect with custom terms");
        repo.git(["bisect", "new", "HEAD"]).expect("mark new");
        repo.git(["bisect", "old", "HEAD~2"]).expect("mark old");
        let expected_commit = repo.rev_parse("HEAD").expect("bisect current head");
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

        assert_eq!(
            detail
                .bisect_state
                .as_ref()
                .map(|state| state.bad_term.as_str()),
            Some("new")
        );
        assert_eq!(
            detail
                .bisect_state
                .as_ref()
                .map(|state| state.good_term.as_str()),
            Some("old")
        );
        assert_eq!(
            detail
                .bisect_state
                .as_ref()
                .and_then(|state| state.current_commit.as_deref()),
            Some(expected_commit.as_str())
        );
        assert_eq!(
            detail
                .bisect_state
                .as_ref()
                .and_then(|state| state.current_summary.as_deref()),
            Some("second")
        );
    }

    #[test]
    fn cli_backend_runs_bisect_start_mark_and_reset_commands() {
        let repo = history_preview_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let bad_commit = repo.rev_parse("HEAD").expect("bad commit");
        let good_commit = repo.rev_parse("HEAD~2").expect("good commit");

        let started = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:start-bisect"),
                repo_id: repo_id.clone(),
                command: GitCommand::StartBisect {
                    commit: bad_commit,
                    term: "bad".to_string(),
                },
            })
            .expect("start bisect succeeds");
        assert!(started.summary.contains("Started bisect"));

        let marked = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:mark-bisect"),
                repo_id: repo_id.clone(),
                command: GitCommand::MarkBisect {
                    commit: good_commit,
                    term: "good".to_string(),
                },
            })
            .expect("mark bisect succeeds");
        assert!(marked.summary.contains("Marked"));

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
            .expect("detail should load");
        assert_eq!(
            detail
                .bisect_state
                .as_ref()
                .and_then(|state| state.current_summary.as_deref()),
            Some("second")
        );

        let reset = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:reset-bisect"),
                repo_id: repo_id.clone(),
                command: GitCommand::ResetBisect,
            })
            .expect("reset bisect succeeds");
        assert_eq!(reset.summary, "Reset active bisect");

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
        assert!(detail.bisect_state.is_none());
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
    fn cli_backend_resets_author_for_selected_commit() {
        let repo = history_preview_repo().expect("fixture repo");
        repo.git(["config", "user.name", "Reset Author"])
            .expect("set user name");
        repo.git(["config", "user.email", "reset@example.com"])
            .expect("set user email");

        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let target = repo.rev_parse("HEAD~1").expect("target commit");

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:amend-commit-reset-author"),
                repo_id,
                command: GitCommand::AmendCommitAttributes {
                    commit: target,
                    reset_author: true,
                    co_author: None,
                },
            })
            .expect("reset author should succeed");

        assert!(outcome.summary.contains("Reset author"));
        assert_eq!(
            stdout_string(
                repo.git_capture(["show", "-s", "--format=%an <%ae>", "HEAD~1"])
                    .expect("author")
            )
            .expect("author text"),
            "Reset Author <reset@example.com>"
        );
    }

    #[test]
    fn cli_backend_sets_co_author_for_head_commit() {
        let repo = history_preview_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let target = repo.rev_parse("HEAD").expect("target commit");

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:amend-commit-set-co-author"),
                repo_id,
                command: GitCommand::AmendCommitAttributes {
                    commit: target,
                    reset_author: false,
                    co_author: Some("Co-authored-by: Pair Dev <pair@example.com>".to_string()),
                },
            })
            .expect("set co-author should succeed");

        assert!(outcome.summary.contains("Set co-author"));
        let body = stdout_string(
            repo.git_capture(["show", "-s", "--format=%B", "HEAD"])
                .expect("body"),
        )
        .expect("body text");
        assert!(body.contains("Co-authored-by: Pair Dev <pair@example.com>"));
    }

    #[test]
    fn cli_backend_resets_author_without_amending_staged_changes() {
        let repo = temp_repo().expect("fixture repo");
        repo.write_file("tracked.txt", "base\n")
            .expect("write tracked");
        repo.git(["add", "tracked.txt"]).expect("stage tracked");
        repo.git(["commit", "-m", "subject"])
            .expect("initial commit");
        repo.write_file("staged.txt", "staged\n")
            .expect("write staged");
        repo.stage("staged.txt").expect("stage extra file");
        repo.git(["config", "user.name", "Reset Author"])
            .expect("set user name");
        repo.git(["config", "user.email", "reset@example.com"])
            .expect("set user email");

        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let target = repo.rev_parse("HEAD").expect("target commit");

        backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:amend-commit-reset-author-staged"),
                repo_id,
                command: GitCommand::AmendCommitAttributes {
                    commit: target,
                    reset_author: true,
                    co_author: None,
                },
            })
            .expect("reset author should succeed");

        assert_eq!(
            stdout_string(
                repo.git_capture(["show", "-s", "--format=%an <%ae>", "HEAD"])
                    .expect("author")
            )
            .expect("author text"),
            "Reset Author <reset@example.com>"
        );
        assert_eq!(
            stdout_string(
                repo.git_capture(["ls-tree", "--name-only", "HEAD"])
                    .expect("tree")
            )
            .expect("tree text"),
            "tracked.txt"
        );
        assert_eq!(
            stdout_string(repo.git_capture(["status", "--short"]).expect("status"))
                .expect("status text"),
            "A  staged.txt"
        );
    }

    #[test]
    fn cli_backend_stacks_co_author_trailers_without_amending_staged_changes() {
        let repo = temp_repo().expect("fixture repo");
        repo.write_file("tracked.txt", "base\n")
            .expect("write tracked");
        repo.git(["add", "tracked.txt"]).expect("stage tracked");
        repo.git([
            "commit",
            "-m",
            "subject",
            "-m",
            "body line",
            "-m",
            "Co-authored-by: Existing <existing@example.com>",
        ])
        .expect("initial commit");
        repo.write_file("staged.txt", "staged\n")
            .expect("write staged");
        repo.stage("staged.txt").expect("stage extra file");

        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let target = repo.rev_parse("HEAD").expect("target commit");

        backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:amend-commit-set-co-author-staged"),
                repo_id,
                command: GitCommand::AmendCommitAttributes {
                    commit: target,
                    reset_author: false,
                    co_author: Some("Co-authored-by: Pair Dev <pair@example.com>".to_string()),
                },
            })
            .expect("set co-author should succeed");

        assert_eq!(
            stdout_string(
                repo.git_capture(["show", "-s", "--format=%B", "HEAD"])
                    .expect("body")
            )
            .expect("body text"),
            "subject\n\nbody line\n\nCo-authored-by: Existing <existing@example.com>\nCo-authored-by: Pair Dev <pair@example.com>"
        );
        assert_eq!(
            stdout_string(
                repo.git_capture(["ls-tree", "--name-only", "HEAD"])
                    .expect("tree")
            )
            .expect("tree text"),
            "tracked.txt"
        );
        assert_eq!(
            stdout_string(repo.git_capture(["status", "--short"]).expect("status"))
                .expect("status text"),
            "A  staged.txt"
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
    fn cli_backend_creates_amend_commit_with_staged_changes() {
        let repo = history_preview_repo().expect("fixture repo");
        repo.append_file("notes.md", "amend line\n")
            .expect("append staged amend change");
        repo.stage("notes.md").expect("stage amend file");

        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:create-amend-commit-with-changes"),
                repo_id,
                command: GitCommand::CreateAmendCommit {
                    original_subject: "second".to_string(),
                    message: "replacement subject".to_string(),
                    include_file_changes: true,
                },
            })
            .expect("create amend! commit with changes should succeed");

        assert!(outcome
            .summary
            .contains("Created amend! commit with changes"));
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
            "amend! second\nadd lib"
        );
        assert_eq!(
            stdout_string(
                repo.git_capture(["log", "--format=%b", "-n", "1"])
                    .expect("body log")
            )
            .expect("body text"),
            "replacement subject"
        );
        assert!(stdout_string(
            repo.git_capture(["show", "HEAD:notes.md"])
                .expect("show notes")
        )
        .expect("notes text")
        .contains("amend line"));
    }

    #[test]
    fn cli_backend_creates_amend_commit_without_file_changes() {
        let repo = history_preview_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let before_notes = stdout_string(
            repo.git_capture(["show", "HEAD:notes.md"])
                .expect("show notes before amend"),
        )
        .expect("notes before amend");

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:create-amend-commit-without-changes"),
                repo_id,
                command: GitCommand::CreateAmendCommit {
                    original_subject: "second".to_string(),
                    message: "message only replacement".to_string(),
                    include_file_changes: false,
                },
            })
            .expect("create amend! commit without changes should succeed");

        assert!(outcome
            .summary
            .contains("Created amend! commit without changes"));
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
            "amend! second\nadd lib"
        );
        assert_eq!(
            stdout_string(
                repo.git_capture(["log", "--format=%b", "-n", "1"])
                    .expect("body log")
            )
            .expect("body text"),
            "message only replacement"
        );
        assert_eq!(
            stdout_string(
                repo.git_capture(["show", "HEAD:notes.md"])
                    .expect("show notes after amend"),
            )
            .expect("notes after amend"),
            before_notes
        );
    }

    #[test]
    fn cli_backend_sets_fixup_message_from_selected_commit() {
        let repo = history_preview_repo().expect("fixture repo");
        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());
        let target = repo.rev_parse("HEAD~1").expect("target commit");

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: JobId::new("git:set-fixup-message-rebase"),
                repo_id: repo_id.clone(),
                command: GitCommand::StartCommitRebase {
                    commit: target,
                    mode: RebaseStartMode::FixupWithMessage,
                },
            })
            .expect("set fixup message flow should succeed");

        assert!(outcome.summary.contains("Set fixup message"));
        assert_eq!(repo.status_porcelain().expect("status"), "");
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
            "add lib\nsecond"
        );
        assert_eq!(
            stdout_string(
                repo.git_capture(["show", "HEAD~1:history.txt"])
                    .expect("show history")
            )
            .expect("history text"),
            "one\ntwo"
        );
        assert!(stdout_string(
            repo.git_capture(["show", "HEAD~1:notes.md"])
                .expect("show notes")
        )
        .expect("notes text")
        .contains("# Notes"));

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
        assert!(!detail.working_tree_state.rebasing);
        assert!(!detail.working_tree_state.merging);
        assert!(detail.working_tree_state.cherry_picking);
        assert!(!detail.working_tree_state.reverting);
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
        assert!(!detail.working_tree_state.rebasing);
        assert!(!detail.working_tree_state.merging);
        assert!(!detail.working_tree_state.cherry_picking);
        assert!(detail.working_tree_state.reverting);
        assert!(detail
            .file_tree
            .iter()
            .any(|item| item.kind == FileStatusKind::Conflicted));
    }

    #[test]
    fn read_working_tree_state_ignores_rebase_internal_cherry_pick_head() {
        let repo = rebase_in_progress_repo().expect("fixture repo");
        let git_dir = resolve_git_path(repo.path(), ".").expect("resolved git dir");
        let cherry_pick_head =
            git_stdout(repo.path(), ["rev-parse", "HEAD"]).expect("current head for overlap");
        let stopped_sha = cherry_pick_head.chars().take(8).collect::<String>();
        fs::write(
            git_dir.join("rebase-merge/stopped-sha"),
            format!("{stopped_sha}\n"),
        )
        .expect("write stopped sha");
        fs::write(
            git_dir.join("CHERRY_PICK_HEAD"),
            format!("{cherry_pick_head}\n"),
        )
        .expect("write cherry-pick head");

        let state = read_working_tree_state(repo.path());

        assert!(state.rebasing);
        assert!(!state.merging);
        assert!(!state.cherry_picking);
        assert!(!state.reverting);
        assert_eq!(read_merge_state(state), MergeState::RebaseInProgress);
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
    fn cli_backend_updates_branch_refs_via_git_update_ref_stdin() {
        let repo = TempRepo::new().expect("fixture repo");
        repo.write_file("shared.txt", "base\n")
            .expect("write base file");
        repo.commit_all("initial").expect("initial commit");
        repo.checkout_new_branch("feature")
            .expect("create feature branch");
        let feature_before = repo.rev_parse("feature").expect("feature before");
        repo.checkout("main").expect("checkout main");
        repo.append_file("shared.txt", "main advance\n")
            .expect("advance main");
        repo.commit_all("main advance")
            .expect("commit main advance");
        let main_after = repo.rev_parse("main").expect("main after");

        let backend = CliGitBackend;
        let updated = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-update-branch-refs"),
                repo_id: RepoId::new(repo.path().display().to_string()),
                command: GitCommand::UpdateBranchRefs {
                    update_commands: format!(
                        "update refs/heads/feature refs/heads/main {feature_before}\n"
                    ),
                },
            })
            .expect("update-ref should succeed");

        assert_eq!(updated.summary, "Updated branch refs");
        assert_eq!(
            repo.rev_parse("feature").expect("feature after"),
            main_after
        );
    }

    #[test]
    fn cli_backend_fetch_selected_repo_auto_forwards_main_branches_only_by_default() {
        let remote = TempRepo::bare().expect("remote fixture");
        let seed = TempRepo::new().expect("seed fixture");
        seed.write_file("shared.txt", "base\n")
            .expect("write base file");
        seed.commit_all("initial").expect("initial commit");
        seed.add_remote("origin", remote.path())
            .expect("add remote");
        seed.push("origin", "HEAD:main").expect("push main");
        seed.checkout_new_branch("feature")
            .expect("create feature branch");
        seed.write_file("feature.txt", "feature base\n")
            .expect("write feature file");
        seed.commit_all("feature initial")
            .expect("commit feature initial");
        seed.push("origin", "HEAD:feature").expect("push feature");

        let repo = TempRepo::clone_from(remote.path()).expect("clone fixture");
        repo.git(["branch", "--set-upstream-to=origin/main", "main"])
            .expect("set main upstream");
        repo.git(["branch", "--track", "feature", "origin/feature"])
            .expect("track feature");
        repo.git(["checkout", "-b", "topic", "main"])
            .expect("checkout topic");
        let local_feature_before = repo.rev_parse("feature").expect("local feature before");

        let upstream = TempRepo::clone_from(remote.path()).expect("upstream fixture");
        upstream
            .git(["checkout", "-B", "main", "origin/main"])
            .expect("checkout main upstream");
        upstream
            .append_file("shared.txt", "remote main advance\n")
            .expect("advance remote main");
        upstream
            .commit_all("remote main advance")
            .expect("commit remote main advance");
        upstream
            .push("origin", "HEAD:main")
            .expect("push remote main");
        let remote_main_after = upstream.rev_parse("HEAD").expect("remote main head");

        upstream
            .git(["checkout", "-B", "feature", "origin/feature"])
            .expect("checkout feature upstream");
        upstream
            .append_file("feature.txt", "remote feature advance\n")
            .expect("advance remote feature");
        upstream
            .commit_all("remote feature advance")
            .expect("commit remote feature advance");
        upstream
            .push("origin", "HEAD:feature")
            .expect("push remote feature");
        let remote_feature_after = upstream.rev_parse("HEAD").expect("remote feature head");

        let backend = CliGitBackend;
        backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-fetch-auto-forward-default"),
                repo_id: RepoId::new(repo.path().display().to_string()),
                command: GitCommand::FetchSelectedRepo,
            })
            .expect("fetch should succeed");

        assert_eq!(
            repo.rev_parse("main").expect("main after"),
            remote_main_after
        );
        assert_eq!(
            repo.rev_parse("feature").expect("feature after"),
            local_feature_before
        );
        assert_eq!(
            repo.rev_parse("origin/feature")
                .expect("origin feature after"),
            remote_feature_after
        );
    }

    #[test]
    fn cli_backend_fetch_selected_repo_skips_auto_forward_for_branch_checked_out_elsewhere() {
        let remote = TempRepo::bare().expect("remote fixture");
        let seed = TempRepo::new().expect("seed fixture");
        seed.write_file("shared.txt", "base\n")
            .expect("write base file");
        seed.commit_all("initial").expect("initial commit");
        seed.add_remote("origin", remote.path())
            .expect("add remote");
        seed.push("origin", "HEAD:main").expect("push main");

        let mut repo = TempRepo::clone_from(remote.path()).expect("clone fixture");
        repo.git(["branch", "--set-upstream-to=origin/main", "main"])
            .expect("set main upstream");
        repo.git(["checkout", "-b", "topic", "main"])
            .expect("checkout topic");
        let local_main_before = repo.rev_parse("main").expect("main before");
        let _worktree_path = repo
            .add_worktree("main-worktree", "main")
            .expect("add main worktree");

        let upstream = TempRepo::clone_from(remote.path()).expect("upstream fixture");
        upstream
            .git(["checkout", "-B", "main", "origin/main"])
            .expect("checkout main upstream");
        upstream
            .append_file("shared.txt", "remote main advance\n")
            .expect("advance remote main");
        upstream
            .commit_all("remote main advance")
            .expect("commit remote main advance");
        upstream
            .push("origin", "HEAD:main")
            .expect("push remote main");
        let remote_main_after = upstream.rev_parse("HEAD").expect("remote main head");

        let backend = CliGitBackend;
        backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-fetch-auto-forward-worktree-skip"),
                repo_id: RepoId::new(repo.path().display().to_string()),
                command: GitCommand::FetchSelectedRepo,
            })
            .expect("fetch should succeed");

        assert_eq!(
            repo.rev_parse("main").expect("main after"),
            local_main_before
        );
        assert_eq!(
            repo.rev_parse("origin/main").expect("origin main after"),
            remote_main_after
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

        let unset = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-unset-upstream"),
                repo_id: repo_id.clone(),
                command: GitCommand::UnsetBranchUpstream {
                    branch_name: "feature".to_string(),
                },
            })
            .expect("unset upstream should succeed");
        assert_eq!(unset.summary, "Unset upstream for feature");
        repo.git_expect_failure([
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "feature@{upstream}",
        ])
        .expect("feature upstream should be cleared");

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
                    force: false,
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
    fn cli_backend_refuses_to_delete_unmerged_branch() {
        let repo = TempRepo::new().expect("fixture repo");
        repo.write_file("shared.txt", "base\n")
            .expect("write base file");
        repo.commit_all("initial").expect("initial commit");

        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-create-unmerged-branch"),
                repo_id: repo_id.clone(),
                command: GitCommand::CreateBranch {
                    branch_name: "feature".to_string(),
                },
            })
            .expect("branch creation should succeed");
        repo.write_file("feature.txt", "feature work\n")
            .expect("write feature file");
        repo.commit_all("feature work")
            .expect("commit feature work");

        backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-checkout-main-for-safe-delete"),
                repo_id: repo_id.clone(),
                command: GitCommand::CheckoutBranch {
                    branch_ref: "main".to_string(),
                },
            })
            .expect("checkout main should succeed");

        let error = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-delete-unmerged-branch"),
                repo_id,
                command: GitCommand::DeleteBranch {
                    branch_name: "feature".to_string(),
                    force: false,
                },
            })
            .expect_err("safe branch deletion should reject unmerged branches");

        let message = if let GitError::OperationFailed { message } = error {
            message
        } else {
            String::new()
        };
        assert!(
            !message.is_empty(),
            "expected operation failure for safe-delete"
        );
        assert!(
            message.contains("not fully merged"),
            "expected safe-delete failure, got: {message}"
        );
        let branch_list = stdout_string(
            repo.git_capture(["branch", "--list"])
                .expect("branch list should load"),
        )
        .expect("branch output");
        assert!(branch_list.contains("feature"));
    }

    #[test]
    fn cli_backend_reports_branch_as_merged_against_existing_main_branch() {
        let repo = TempRepo::new().expect("fixture repo");
        repo.write_file("shared.txt", "base\n")
            .expect("write base file");
        repo.commit_all("initial").expect("initial commit");

        repo.checkout_new_branch("feature")
            .expect("create feature branch");
        repo.write_file("feature.txt", "merged work\n")
            .expect("write feature file");
        repo.commit_all("feature work")
            .expect("commit feature work");
        repo.checkout("main").expect("checkout main");
        repo.git(["merge", "--no-edit", "feature"])
            .expect("merge feature into main");
        repo.checkout_new_branch("other")
            .expect("checkout other branch");

        let backend = CliGitBackend;
        let merged = backend
            .read_branch_merge_status(BranchMergeStatusRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
                branch_name: "feature".to_string(),
            })
            .expect("merge status should load");

        assert!(merged, "feature should count as merged via main");
    }

    #[test]
    fn cli_backend_reports_branch_as_unmerged_when_commit_is_unique() {
        let repo = TempRepo::new().expect("fixture repo");
        repo.write_file("shared.txt", "base\n")
            .expect("write base file");
        repo.commit_all("initial").expect("initial commit");

        repo.checkout_new_branch("feature")
            .expect("create feature branch");
        repo.write_file("feature.txt", "unique work\n")
            .expect("write feature file");
        repo.commit_all("feature work")
            .expect("commit feature work");
        repo.checkout("main").expect("checkout main");

        let backend = CliGitBackend;
        let merged = backend
            .read_branch_merge_status(BranchMergeStatusRequest {
                repo_id: RepoId::new(repo.path().display().to_string()),
                branch_name: "feature".to_string(),
            })
            .expect("merge status should load");

        assert!(!merged, "feature should remain unmerged");
    }

    #[test]
    fn cli_backend_fast_forwards_current_branch_from_upstream() {
        let remote = TempRepo::bare().expect("remote fixture");
        let seed = TempRepo::new().expect("seed fixture");
        seed.write_file("shared.txt", "base\n")
            .expect("write tracked file");
        seed.commit_all("initial").expect("seed initial commit");
        seed.add_remote("origin", remote.path())
            .expect("attach remote");
        seed.push("origin", "HEAD:main").expect("push main");

        let repo = TempRepo::clone_from(remote.path()).expect("clone fixture");
        let upstream = TempRepo::clone_from(remote.path()).expect("upstream fixture");
        upstream
            .append_file("shared.txt", "remote\n")
            .expect("append remote change");
        upstream
            .commit_all("remote change")
            .expect("commit remote change");
        upstream
            .push("origin", "HEAD:main")
            .expect("push updated main");
        repo.fetch("origin").expect("fetch origin");

        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-fast-forward-upstream"),
                repo_id: repo_id.clone(),
                command: GitCommand::FastForwardCurrentBranchFromUpstream {
                    upstream_ref: "origin/main".to_string(),
                },
            })
            .expect("fast-forward should succeed");

        assert_eq!(outcome.repo_id, repo_id);
        assert_eq!(
            outcome.summary,
            "Fast-forwarded current branch from origin/main"
        );
        assert_eq!(
            stdout_string(
                repo.git_capture(["show", "-s", "--format=%s", "HEAD"])
                    .expect("head subject")
            )
            .expect("head subject text"),
            "remote change"
        );
        assert_eq!(
            repo.rev_parse("HEAD").expect("head"),
            upstream.rev_parse("HEAD").expect("upstream head")
        );
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
                    track: true,
                },
            })
            .expect("create branch from remote should succeed");
        assert_eq!(
            created.summary,
            "Created feature-copy tracking origin/feature"
        );
        assert_eq!(repo.current_branch().expect("current branch"), "main");
        assert_eq!(
            stdout_string(
                repo.git_capture([
                    "rev-parse",
                    "--abbrev-ref",
                    "--symbolic-full-name",
                    "feature-copy@{u}",
                ])
                .expect("feature-copy upstream ref"),
            )
            .expect("feature-copy upstream text"),
            "origin/feature"
        );

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

        let created_without_tracking = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-create-branch-from-ref-no-track"),
                repo_id: repo_id.clone(),
                command: GitCommand::CreateBranchFromRef {
                    branch_name: "feature-custom".to_string(),
                    start_point: "origin/feature".to_string(),
                    track: false,
                },
            })
            .expect("create branch from remote without tracking should succeed");
        assert_eq!(
            created_without_tracking.summary,
            "Created feature-custom from origin/feature without tracking"
        );
        repo.git_expect_failure([
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "feature-custom@{u}",
        ])
        .expect("custom local branch should not track upstream");

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
    fn cli_backend_merges_selected_ref_into_current_branch() {
        let repo = TempRepo::new().expect("fixture repo");
        repo.write_file("shared.txt", "base\n")
            .expect("write base file");
        repo.commit_all("initial").expect("initial commit");

        repo.checkout_new_branch("feature")
            .expect("create feature branch");
        repo.write_file("feature.txt", "feature\n")
            .expect("write feature file");
        repo.commit_all("feature change").expect("feature commit");

        repo.checkout("main").expect("checkout main");
        repo.write_file("main.txt", "main\n")
            .expect("write main file");
        repo.commit_all("main change").expect("main commit");

        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-merge-ref"),
                repo_id: repo_id.clone(),
                command: GitCommand::MergeRefIntoCurrent {
                    target_ref: "feature".to_string(),
                    variant: MergeVariant::Regular,
                },
            })
            .expect("merge should succeed");

        assert_eq!(outcome.repo_id, repo_id);
        assert_eq!(outcome.summary, "Merged feature into current branch");
        assert!(repo.path().join("feature.txt").exists());
        assert_eq!(repo.current_branch().expect("current branch"), "main");
    }

    #[test]
    fn cli_backend_force_deletes_unmerged_branch() {
        let repo = TempRepo::new().expect("fixture repo");
        repo.write_file("shared.txt", "base\n")
            .expect("write base file");
        repo.commit_all("initial").expect("initial commit");

        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-create-force-delete-branch"),
                repo_id: repo_id.clone(),
                command: GitCommand::CreateBranch {
                    branch_name: "feature".to_string(),
                },
            })
            .expect("branch creation should succeed");
        repo.write_file("feature.txt", "feature work\n")
            .expect("write feature file");
        repo.commit_all("feature work")
            .expect("commit feature work");
        repo.checkout("main").expect("checkout main");

        let deleted = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-force-delete-branch"),
                repo_id: repo_id.clone(),
                command: GitCommand::DeleteBranch {
                    branch_name: "feature".to_string(),
                    force: true,
                },
            })
            .expect("force branch delete should succeed");

        assert_eq!(deleted.summary, "Force-deleted feature");
        let branch_list = stdout_string(
            repo.git_capture(["branch", "--list"])
                .expect("branch list should load"),
        )
        .expect("branch output");
        assert!(!branch_list.contains("feature"));
    }

    #[test]
    fn merge_ref_args_match_branch_merge_variants() {
        let regular = merge_ref_args("feature", MergeVariant::Regular);
        let fast_forward = merge_ref_args("feature", MergeVariant::FastForward);
        let non_fast_forward = merge_ref_args("feature", MergeVariant::NoFastForward);
        let squash = merge_ref_args("feature", MergeVariant::Squash);

        assert_eq!(
            regular,
            vec![
                OsString::from("merge"),
                OsString::from("--no-edit"),
                OsString::from("feature"),
            ]
        );
        assert_eq!(
            fast_forward,
            vec![
                OsString::from("merge"),
                OsString::from("--no-edit"),
                OsString::from("--ff"),
                OsString::from("feature"),
            ]
        );
        assert_eq!(
            non_fast_forward,
            vec![
                OsString::from("merge"),
                OsString::from("--no-edit"),
                OsString::from("--no-ff"),
                OsString::from("feature"),
            ]
        );
        assert_eq!(
            squash,
            vec![
                OsString::from("merge"),
                OsString::from("--no-edit"),
                OsString::from("--squash"),
                OsString::from("--ff"),
                OsString::from("feature"),
            ]
        );
    }

    #[test]
    fn cached_git_config_get_and_drop_cache_match_upstream_behavior() {
        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let runner_calls = call_count.clone();
        let config =
            CachedGitConfig::with_runner(Path::new("/tmp/repo"), move |_repo_path, args| {
                runner_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                assert_eq!(
                    args,
                    &[
                        OsString::from("config"),
                        OsString::from("--get"),
                        OsString::from("--null"),
                        OsString::from("commit.gpgsign"),
                    ]
                );
                Ok(" true ".to_string())
            });

        assert_eq!(config.get("commit.gpgsign"), "true");
        assert_eq!(config.get("commit.gpgsign"), "true");
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);

        config.drop_cache();

        assert_eq!(config.get("commit.gpgsign"), "true");
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[test]
    fn cached_git_config_get_general_splits_args_and_caches_by_args() {
        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let runner_calls = call_count.clone();
        let config =
            CachedGitConfig::with_runner(Path::new("/tmp/repo"), move |_repo_path, args| {
                runner_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                assert_eq!(
                    args,
                    &[
                        OsString::from("config"),
                        OsString::from("--local"),
                        OsString::from("--get-regexp"),
                        OsString::from("gitflow.prefix"),
                    ]
                );
                Ok("gitflow.prefix.feature feature/\n".to_string())
            });

        assert_eq!(
            config.get_general("--local --get-regexp gitflow.prefix"),
            "gitflow.prefix.feature feature/"
        );
        assert_eq!(
            config.get_general("--local --get-regexp gitflow.prefix"),
            "gitflow.prefix.feature feature/"
        );
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn cached_git_config_get_bool_matches_upstream_truthy_values() {
        let scenarios = [
            (
                Err(GitError::OperationFailed {
                    message: "missing".to_string(),
                }),
                false,
            ),
            (Ok("True".to_string()), true),
            (Ok("ON".to_string()), true),
            (Ok("YeS".to_string()), true),
            (Ok("1".to_string()), true),
            (Ok("false".to_string()), false),
        ];

        for (result, expected) in scenarios {
            let config =
                CachedGitConfig::with_runner(Path::new("/tmp/repo"), move |_repo_path, args| {
                    assert_eq!(
                        args,
                        &[
                            OsString::from("config"),
                            OsString::from("--get"),
                            OsString::from("--null"),
                            OsString::from("commit.gpgsign"),
                        ]
                    );
                    result.clone()
                });

            assert_eq!(config.get_bool("commit.gpgsign"), expected);
        }
    }

    #[test]
    fn git_flow_start_args_match_upstream_shape() {
        assert_eq!(
            git_flow_start_args(GitFlowBranchType::Feature, "test"),
            vec![
                OsString::from("flow"),
                OsString::from("feature"),
                OsString::from("start"),
                OsString::from("test"),
            ]
        );
    }

    #[test]
    fn resolve_git_flow_finish_parts_requires_matching_configured_prefix() {
        let repo = TempRepo::new().expect("fixture repo");
        repo.write_file("tracked.txt", "base\n")
            .expect("write tracked file");
        repo.commit_all("initial").expect("initial commit");
        repo.git(["config", "gitflow.prefix.feature", "feature/"])
            .expect("configure git-flow prefix");

        assert_eq!(
            resolve_git_flow_finish_parts(repo.path(), "feature/mybranch")
                .expect("git-flow branch should resolve"),
            ("feature".to_string(), "mybranch".to_string())
        );
        assert_eq!(
            resolve_git_flow_finish_parts(repo.path(), "mybranch")
                .expect_err("plain branch should fail")
                .to_string(),
            "git operation failed: This does not seem to be a git flow branch"
        );
    }

    #[test]
    fn cli_backend_rebases_current_branch_onto_selected_ref() {
        let repo = TempRepo::new().expect("fixture repo");
        repo.write_file("shared.txt", "base\n")
            .expect("write base file");
        repo.commit_all("initial").expect("initial commit");

        repo.checkout_new_branch("feature")
            .expect("create feature branch");
        repo.write_file("feature.txt", "feature\n")
            .expect("write feature file");
        repo.commit_all("feature change").expect("feature commit");

        repo.checkout("main").expect("checkout main");
        repo.write_file("main.txt", "main\n")
            .expect("write main file");
        repo.commit_all("main change").expect("main commit");

        repo.checkout("feature").expect("checkout feature");

        let backend = CliGitBackend;
        let repo_id = RepoId::new(repo.path().display().to_string());

        let outcome = backend
            .run_command(GitCommandRequest {
                job_id: super_lazygit_core::JobId::new("job-rebase-ref"),
                repo_id: repo_id.clone(),
                command: GitCommand::RebaseCurrentOntoRef {
                    target_ref: "main".to_string(),
                },
            })
            .expect("rebase should succeed");

        assert_eq!(outcome.repo_id, repo_id);
        assert_eq!(outcome.summary, "Rebased current branch onto main");
        assert_eq!(
            stdout_string(
                repo.git_capture(["show", "-s", "--format=%s", "HEAD"])
                    .expect("head subject")
            )
            .expect("head subject text"),
            "feature change"
        );
        assert_eq!(
            stdout_string(
                repo.git_capture(["show", "-s", "--format=%s", "HEAD~1"])
                    .expect("previous subject")
            )
            .expect("previous subject text"),
            "main change"
        );
        assert_eq!(repo.current_branch().expect("current branch"), "feature");
    }

    #[test]
    fn cli_backend_reads_remotes_with_metadata_and_branch_counts() {
        let origin = TempRepo::bare().expect("origin fixture");
        let alpha = TempRepo::bare().expect("alpha fixture");
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
        repo.add_remote("Alpha", alpha.path())
            .expect("attach alpha");
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

        assert_eq!(
            detail
                .remotes
                .iter()
                .map(|remote| remote.name.as_str())
                .collect::<Vec<_>>(),
            vec!["origin", "Alpha", "mirror"]
        );

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

        let alpha_remote = detail
            .remotes
            .iter()
            .find(|remote| remote.name == "Alpha")
            .expect("alpha remote present");
        assert_eq!(alpha_remote.branch_count, 0);
        assert_eq!(
            alpha_remote.fetch_url,
            stdout_string(
                repo.git_capture(["remote", "get-url", "Alpha"])
                    .expect("alpha fetch url")
            )
            .expect("alpha fetch output")
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
    fn parse_remote_urls_by_name_groups_config_entries() {
        let output = "\
remote.origin.url https://example.com/origin.git\n\
remote.origin.url git@example.com:origin.git\n\
remote.Alpha.url https://example.com/alpha.git\n\
garbage\n";

        assert_eq!(
            parse_remote_urls_by_name(output),
            BTreeMap::from([
                (
                    "Alpha".to_string(),
                    vec!["https://example.com/alpha.git".to_string()]
                ),
                (
                    "origin".to_string(),
                    vec![
                        "https://example.com/origin.git".to_string(),
                        "git@example.com:origin.git".to_string(),
                    ]
                ),
            ])
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
    fn parse_tag_listing_matches_lazygit_loader_cases() {
        assert!(parse_tag_listing("").is_empty());
        assert_eq!(
            parse_tag_listing("tag1 this is my message\ntag2\ntag3 this is my other message\n"),
            vec![
                ("tag1".to_string(), "this is my message".to_string()),
                ("tag2".to_string(), String::new()),
                ("tag3".to_string(), "this is my other message".to_string()),
            ]
        );
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

        assert_eq!(detail.tags.len(), 2);
        assert_eq!(
            detail
                .tags
                .iter()
                .map(|tag| (tag.name.as_str(), tag.summary.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("snapshot", "snapshot commit"),
                ("v1.0.0", "release v1.0.0"),
            ]
        );

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
    fn parsed_patch_exposes_hunk_navigation_and_line_numbers() {
        let patch = parse_patch(
            "\
diff --git a/file.txt b/file.txt
index 1111111..2222222 100644
--- a/file.txt
+++ b/file.txt
@@ -2,3 +2,4 @@ heading
 line two
-old three
+new three
 line four
+line five
@@ -10,2 +11 @@
-old ten
-old eleven
+new ten
",
        )
        .expect("patch should parse");

        assert_eq!(patch.hunk_count(), 2);
        assert_eq!(patch.line_count(), 14);
        assert_eq!(patch.hunk_start_idx(0), 4);
        assert_eq!(patch.hunk_end_idx(0), 9);
        assert_eq!(patch.hunk_old_start_for_line(6), 2);
        assert_eq!(patch.hunk_containing_line(11), Some(1));
        assert_eq!(patch.line_number_of_line(0), 1);
        assert_eq!(patch.line_number_of_line(4), 2);
        assert_eq!(patch.line_number_of_line(6), 3);
        assert_eq!(patch.line_number_of_line(12), 11);
        assert!(patch.contains_changes());
    }

    #[test]
    fn parsed_patch_supports_change_lookup_and_line_adjustment() {
        let patch = parse_patch(
            "\
diff --git a/file.txt b/file.txt
index 1111111..2222222 100644
--- a/file.txt
+++ b/file.txt
@@ -2,3 +2,4 @@
 line two
-old three
+new three
 line four
+line five
@@ -10,2 +11 @@
-old ten
-old eleven
+new ten
",
        )
        .expect("patch should parse");

        assert_eq!(patch.get_next_change_idx(0), 6);
        assert_eq!(
            patch.get_next_change_idx_of_same_included_state(0, &[6, 7], true),
            (6, true)
        );
        assert_eq!(
            patch.get_next_change_idx_of_same_included_state(8, &[6, 7], true),
            (7, true)
        );
        assert_eq!(patch.adjust_line_number(1), 1);
        assert_eq!(patch.adjust_line_number(3), 2);
        assert_eq!(patch.adjust_line_number(11), 11);
        assert_eq!(
            patch.format_range_plain(4, 7),
            "@@ -2,3 +2,4 @@\n line two\n-old three\n+new three"
        );
    }

    #[test]
    fn parsed_patch_detects_single_whole_file_hunk() {
        let additions = parse_patch(
            "\
diff --git a/new.txt b/new.txt
new file mode 100644
--- /dev/null
+++ b/new.txt
@@ -0,0 +1,2 @@
+one
+two
",
        )
        .expect("patch should parse");
        let mixed = parse_patch(
            "\
diff --git a/file.txt b/file.txt
--- a/file.txt
+++ b/file.txt
@@ -1,2 +1,2 @@
 line one
-old two
+new two
",
        )
        .expect("patch should parse");

        assert!(additions.is_single_hunk_for_whole_file());
        assert!(!mixed.is_single_hunk_for_whole_file());
        assert!(additions.format_plain().contains("new file mode 100644"));
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

    fn filtered_stash_repo() -> std::io::Result<super_lazygit_test_support::TempRepo> {
        let repo = temp_repo()?;
        repo.write_file("src/only-first.txt", "base\n")?;
        repo.write_file("docs/only-second.txt", "base\n")?;
        repo.commit_all("initial")?;

        repo.write_file("src/only-first.txt", "first stash\n")?;
        repo.git(["stash", "push", "-m", "first path"])?;

        repo.write_file("docs/only-second.txt", "second stash\n")?;
        repo.git(["stash", "push", "-m", "second path"])?;

        Ok(repo)
    }

    fn staged_untracked_with_unstaged_repo() -> std::io::Result<super_lazygit_test_support::TempRepo>
    {
        let repo = temp_repo()?;
        repo.write_file("tracked.txt", "base\n")?;
        repo.commit_all("initial")?;
        repo.write_file("staged-untracked.txt", "new file\n")?;
        repo.stage("staged-untracked.txt")?;
        repo.write_file("tracked.txt", "base changed\n")?;
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

    fn blame_repo() -> std::io::Result<super_lazygit_test_support::TempRepo> {
        let repo = temp_repo()?;
        repo.write_file("notes.txt", "alpha\nbeta\ngamma\n")?;
        repo.commit_all("initial")?;
        repo.write_file("notes.txt", "alpha\nbeta updated\ngamma\n")?;
        repo.commit_all("update second line")?;
        Ok(repo)
    }
}
