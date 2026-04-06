// Ported from ./references/lazygit-master/pkg/gui/types/refresh.go

#[derive(Clone, Copy)]
pub enum RefreshableView {
    Commits,
    RebaseCommits,
    SubCommits,
    Branches,
    Files,
    Stash,
    Reflog,
    Tags,
    Remotes,
    Worktrees,
    Status,
    Submodules,
    Staging,
    PatchBuilding,
    MergeConflicts,
    CommitFiles,
    BisectInfo,
}

#[derive(Clone, Copy)]
pub enum RefreshMode {
    Sync,
    Async,
    BlockUI,
}

pub struct RefreshOptions {
    pub then: Option<Box<dyn Fn()>>,
    pub scope: Vec<RefreshableView>,
    pub mode: RefreshMode,
    pub keep_branch_selection_index: bool,
}
