pub struct ControllerCommon {
    pub helper_common: HelperCommon,
}

pub struct HelperCommon {
    pub context: String,
}

impl HelperCommon {
    pub fn new() -> Self {
        Self {
            context: String::new(),
        }
    }
}

impl Default for HelperCommon {
    fn default() -> Self {
        Self::new()
    }
}

impl ControllerCommon {
    pub fn new() -> Self {
        Self {
            helper_common: HelperCommon::new(),
        }
    }
}

impl Default for ControllerCommon {
    fn default() -> Self {
        Self::new()
    }
}

pub trait IGetHelpers {
    fn helpers(&self) -> &Helpers;
}

pub struct Helpers {
    pub refs: RefsHelper,
    pub branches: BranchesHelper,
    pub merge_and_rebase: MergeAndRebaseHelper,
    pub diff: DiffHelper,
    pub worktree: WorktreeHelper,
    pub sub_commits: SubCommitsHelper,
    pub upstream: UpstreamHelper,
    pub suggestions: SuggestionsHelper,
    pub tags: TagsHelper,
}

pub struct RefsHelper;
pub struct BranchesHelper;
pub struct MergeAndRebaseHelper;
pub struct DiffHelper;
pub struct WorktreeHelper;
pub struct SubCommitsHelper;
pub struct UpstreamHelper;
pub struct SuggestionsHelper;
pub struct TagsHelper;
