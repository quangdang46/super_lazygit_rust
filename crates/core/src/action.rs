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
    PushCurrentBranch,
    SwitchRepoSubview(RepoSubview),
    ApplyWorkspaceScan(crate::state::WorkspaceState),
}
