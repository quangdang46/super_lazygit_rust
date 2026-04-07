// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/helpers.go

pub struct HelperCommon {
    pub common: Common,
}

pub struct Common;

pub struct Helpers {
    pub refs: RefsHelper,
    pub bisect: BisectHelper,
    pub suggestions: SuggestionsHelper,
    pub files: FilesHelper,
    pub working_tree: WorkingTreeHelper,
    pub branches_helper: BranchesHelper,
    pub tags: TagsHelper,
    pub merge_and_rebase: MergeAndRebaseHelper,
    pub merge_conflicts: MergeConflictsHelper,
    pub cherry_pick: CherryPickHelper,
    pub host: HostHelper,
    pub patch_building: PatchBuildingHelper,
    pub staging: StagingHelper,
    pub gpg: GpgHelper,
    pub upstream: UpstreamHelper,
    pub amend_helper: AmendHelper,
    pub fixup_helper: FixupHelper,
    pub commits: CommitsHelper,
    pub suspend_resume: SuspendResumeHelper,
    pub snake: SnakeHelper,
    pub diff: DiffHelper,
    pub repos: ReposHelper,
    pub record_directory: RecordDirectoryHelper,
    pub update: UpdateHelper,
    pub window: WindowHelper,
    pub view: ViewHelper,
    pub refresh: RefreshHelper,
    pub confirmation: ConfirmationHelper,
    pub mode: ModeHelper,
    pub app_status: AppStatusHelper,
    pub inline_status: InlineStatusHelper,
    pub window_arrangement: WindowArrangementHelper,
    pub search: SearchHelper,
    pub worktree: WorktreeHelper,
    pub sub_commits: SubCommitsHelper,
}

pub struct RefsHelper;
pub struct SuggestionsHelper;
pub struct FilesHelper;
pub struct WorkingTreeHelper;
pub struct TagsHelper;
pub struct MergeAndRebaseHelper;
pub struct MergeConflictsHelper;
pub struct CherryPickHelper;
pub struct HostHelper;
pub struct PatchBuildingHelper;
pub struct StagingHelper;
pub struct GpgHelper;
pub struct UpstreamHelper;
pub struct FixupHelper;
pub struct CommitsHelper;
pub struct SuspendResumeHelper;
pub struct SnakeHelper;
pub struct ReposHelper;
pub struct RecordDirectoryHelper;
pub struct UpdateHelper;
pub struct WindowHelper;
pub struct ViewHelper;
pub struct RefreshHelper;
pub struct ConfirmationHelper;
pub struct InlineStatusHelper;
pub struct WindowArrangementHelper;
pub struct SearchHelper;
pub struct SubCommitsHelper;
pub struct BisectHelper;
pub struct BranchesHelper;
pub struct AmendHelper;
pub struct DiffHelper;
pub struct ModeHelper;
pub struct AppStatusHelper;
pub struct WorktreeHelper;

impl Helpers {
    pub fn new_stub() -> Self {
        Self {
            refs: RefsHelper,
            bisect: BisectHelper,
            suggestions: SuggestionsHelper,
            files: FilesHelper,
            working_tree: WorkingTreeHelper,
            branches_helper: BranchesHelper,
            tags: TagsHelper,
            merge_and_rebase: MergeAndRebaseHelper,
            merge_conflicts: MergeConflictsHelper,
            cherry_pick: CherryPickHelper,
            host: HostHelper,
            patch_building: PatchBuildingHelper,
            staging: StagingHelper,
            gpg: GpgHelper,
            upstream: UpstreamHelper,
            amend_helper: AmendHelper,
            fixup_helper: FixupHelper,
            commits: CommitsHelper,
            suspend_resume: SuspendResumeHelper,
            snake: SnakeHelper,
            diff: DiffHelper,
            repos: ReposHelper,
            record_directory: RecordDirectoryHelper,
            update: UpdateHelper,
            window: WindowHelper,
            view: ViewHelper,
            refresh: RefreshHelper,
            confirmation: ConfirmationHelper,
            mode: ModeHelper,
            app_status: AppStatusHelper,
            inline_status: InlineStatusHelper,
            window_arrangement: WindowArrangementHelper,
            search: SearchHelper,
            worktree: WorktreeHelper,
            sub_commits: SubCommitsHelper,
        }
    }
}
