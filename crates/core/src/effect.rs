use serde::{Deserialize, Serialize};

use crate::state::{JobId, RepoId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Effect {
    StartRepoScan,
    RefreshRepoSummary { repo_id: RepoId },
    LoadRepoDetail { repo_id: RepoId },
    RunGitCommand(GitCommandRequest),
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
pub enum GitCommand {
    StageSelection,
    CommitStaged { message: String },
    AmendHead { message: Option<String> },
    CheckoutBranch { branch_ref: String },
    FetchSelectedRepo,
    PullCurrentBranch,
    PushCurrentBranch,
    RefreshSelectedRepo,
}
