use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use super_lazygit_core::{
    ComparisonTarget, Diagnostics, DiagnosticsSnapshot, DiffModel, GitCommandRequest, RepoDetail,
    RepoId, RepoSummary,
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
        Self::new(NoopGitBackend)
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
            primary_backend: GitBackendKind::Gix,
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
        super_lazygit_core::GitCommand::StageSelection => "stage_selection",
        super_lazygit_core::GitCommand::CommitStaged { .. } => "commit_staged",
        super_lazygit_core::GitCommand::PushCurrentBranch => "push_current_branch",
        super_lazygit_core::GitCommand::RefreshSelectedRepo => "refresh_selected_repo",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super_lazygit_core::{DiffModel, GitCommand, GitCommandRequest, RepoId};

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
                kind: GitBackendKind::Gix,
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
            GitBackendKind::Gix
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
    fn noop_backend_returns_empty_workspace_scan() {
        let mut facade = GitFacade::default();

        let result = facade
            .scan_workspace(WorkspaceScanRequest {
                root: Some(PathBuf::from("/tmp/workspace")),
            })
            .expect("noop workspace scan should be harmless");

        assert_eq!(result.root, Some(PathBuf::from("/tmp/workspace")));
        assert!(result.repo_ids.is_empty());
    }
}
