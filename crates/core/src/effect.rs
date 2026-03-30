use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::state::{ComparisonTarget, DiffPresentation, JobId, RepoId, ResetMode, SelectedHunk};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Effect {
    StartRepoScan,
    ConfigureWatcher {
        repo_ids: Vec<RepoId>,
    },
    ScheduleWatcherDebounce,
    RefreshRepoSummaries {
        repo_ids: Vec<RepoId>,
    },
    RefreshRepoSummary {
        repo_id: RepoId,
    },
    LoadRepoDetail {
        repo_id: RepoId,
        selected_path: Option<PathBuf>,
        diff_presentation: DiffPresentation,
    },
    LoadRepoDiff {
        repo_id: RepoId,
        comparison_target: Option<ComparisonTarget>,
        compare_with: Option<ComparisonTarget>,
        selected_path: Option<PathBuf>,
        diff_presentation: DiffPresentation,
    },
    OpenEditor {
        cwd: PathBuf,
        target: PathBuf,
    },
    RunGitCommand(GitCommandRequest),
    RunPatchSelection(PatchSelectionJob),
    PersistCache,
    PersistConfig,
    ScheduleRender,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitCommandRequest {
    pub job_id: JobId,
    pub repo_id: RepoId,
    pub command: GitCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RebaseStartMode {
    Interactive,
    Amend,
    Fixup,
    Reword { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GitCommand {
    StageSelection,
    StageFile {
        path: PathBuf,
    },
    DiscardFile {
        path: PathBuf,
    },
    UnstageFile {
        path: PathBuf,
    },
    CommitStaged {
        message: String,
    },
    CommitStagedNoVerify {
        message: String,
    },
    CommitStagedWithEditor,
    AmendHead {
        message: Option<String>,
    },
    RewordCommitWithEditor {
        commit: String,
    },
    StartCommitRebase {
        commit: String,
        mode: RebaseStartMode,
    },
    CherryPickCommit {
        commit: String,
    },
    RevertCommit {
        commit: String,
    },
    ResetToCommit {
        mode: ResetMode,
        target: String,
    },
    RestoreSnapshot {
        target: String,
    },
    ContinueRebase,
    AbortRebase,
    SkipRebase,
    CreateBranch {
        branch_name: String,
    },
    CheckoutBranch {
        branch_ref: String,
    },
    RenameBranch {
        branch_name: String,
        new_name: String,
    },
    DeleteBranch {
        branch_name: String,
    },
    CreateStash {
        message: Option<String>,
        include_untracked: bool,
    },
    ApplyStash {
        stash_ref: String,
    },
    DropStash {
        stash_ref: String,
    },
    CreateWorktree {
        path: PathBuf,
        branch_ref: String,
    },
    RemoveWorktree {
        path: PathBuf,
    },
    SetBranchUpstream {
        branch_name: String,
        upstream_ref: String,
    },
    FetchSelectedRepo,
    PullCurrentBranch,
    PushCurrentBranch,
    NukeWorkingTree,
    RefreshSelectedRepo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchSelectionJob {
    pub job_id: JobId,
    pub repo_id: RepoId,
    pub path: PathBuf,
    pub mode: PatchApplicationMode,
    pub hunks: Vec<SelectedHunk>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchApplicationMode {
    Stage,
    Unstage,
}
