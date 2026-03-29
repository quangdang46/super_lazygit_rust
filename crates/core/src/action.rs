use serde::{Deserialize, Serialize};

use crate::state::{ModalKind, RepoId, RepoSubview};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    EnterRepoMode { repo_id: RepoId },
    LeaveRepoMode,
    SelectNextRepo,
    SelectPreviousRepo,
    SetFocusedPane(crate::state::PaneId),
    OpenModal { kind: ModalKind, title: String },
    CloseTopModal,
    RefreshSelectedRepo,
    RefreshVisibleRepos,
    StageSelection,
    CommitStaged { message: String },
    AmendHead { message: Option<String> },
    CheckoutBranch { branch_ref: String },
    FetchSelectedRepo,
    PullCurrentBranch,
    PushCurrentBranch,
    SwitchRepoSubview(RepoSubview),
    ApplyWorkspaceScan(crate::state::WorkspaceState),
}
