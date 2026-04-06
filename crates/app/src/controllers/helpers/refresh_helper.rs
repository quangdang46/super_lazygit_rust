// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/refresh_helper.go

use std::sync::{Arc, Mutex};
use std::time::Instant;

pub struct RefreshHelper {
    common: HelperCommon,
    refs_helper: Arc<RefsHelper>,
    merge_and_rebase_helper: Arc<MergeAndRebaseHelper>,
    patch_building_helper: Arc<PatchBuildingHelper>,
    staging_helper: Arc<StagingHelper>,
    merge_conflicts_helper: Arc<MergeConflictsHelper>,
    worktree_helper: Arc<WorktreeHelper>,
    search_helper: Arc<SearchHelper>,
}

pub struct HelperCommon;

pub struct RefsHelper;
pub struct MergeAndRebaseHelper;
pub struct PatchBuildingHelper;
pub struct StagingHelper;
pub struct MergeConflictsHelper;
pub struct WorktreeHelper;
pub struct SearchHelper;

pub struct RefreshOptions {
    pub mode: RefreshMode,
    pub scope: Vec<RefreshableView>,
    pub then: Option<Box<dyn FnOnce() + Send + Sync>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RefreshMode {
    Sync,
    Async,
    BlockUi,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RefreshableView {
    Commits,
    Branches,
    Files,
    Stash,
    Reflog,
    Tags,
    Remotes,
    Worktrees,
    Status,
    BisectInfo,
    Staging,
    Submodules,
    SubCommits,
    CommitFiles,
    MergeConflicts,
    PatchBuilding,
    RebaseCommits,
}

impl RefreshHelper {
    pub fn new(
        common: HelperCommon,
        refs_helper: Arc<RefsHelper>,
        merge_and_rebase_helper: Arc<MergeAndRebaseHelper>,
        patch_building_helper: Arc<PatchBuildingHelper>,
        staging_helper: Arc<StagingHelper>,
        merge_conflicts_helper: Arc<MergeConflictsHelper>,
        worktree_helper: Arc<WorktreeHelper>,
        search_helper: Arc<SearchHelper>,
    ) -> Self {
        Self {
            common,
            refs_helper,
            merge_and_rebase_helper,
            patch_building_helper,
            staging_helper,
            merge_conflicts_helper,
            worktree_helper,
            search_helper,
        }
    }

    pub fn refresh(&self, options: RefreshOptions) {
        if options.mode == RefreshMode::Async {
            if options.then.is_some() {
                panic!("RefreshOptions.Then doesn't work with mode ASYNC");
            }
        }

        let start = Instant::now();

        self.refresh_internal(options.clone());

        let _elapsed = start.elapsed();
    }

    fn refresh_internal(&self, options: RefreshOptions) {
        let scope_set: Vec<RefreshableView> = if options.scope.is_empty() {
            vec![
                RefreshableView::Commits,
                RefreshableView::Branches,
                RefreshableView::Files,
                RefreshableView::Stash,
                RefreshableView::Reflog,
                RefreshableView::Tags,
                RefreshableView::Remotes,
                RefreshableView::Worktrees,
                RefreshableView::Status,
                RefreshableView::BisectInfo,
                RefreshableView::Staging,
            ]
        } else {
            options.scope.clone()
        };

        let has_commits = scope_set.contains(&RefreshableView::Commits);
        let has_branches = scope_set.contains(&RefreshableView::Branches);
        let has_reflog = scope_set.contains(&RefreshableView::Reflog);
        let has_bisect_info = scope_set.contains(&RefreshableView::BisectInfo);

        if has_commits || has_branches || has_reflog || has_bisect_info {
            self.refresh_commits_and_branches(&scope_set, &options);
        }

        if scope_set.contains(&RefreshableView::RebaseCommits) {
            self.refresh_rebase_commits();
        }

        if scope_set.contains(&RefreshableView::SubCommits) {
            self.refresh_sub_commits_with_limit();
        }

        if scope_set.contains(&RefreshableView::CommitFiles) && !has_commits {
            self.refresh_commit_files_context();
        }

        if scope_set.contains(&RefreshableView::Files)
            || scope_set.contains(&RefreshableView::Submodules)
        {
            self.refresh_files_and_submodules();
        }

        if scope_set.contains(&RefreshableView::Stash) {
            self.refresh_stash_entries();
        }

        if scope_set.contains(&RefreshableView::Tags) {
            self.refresh_tags();
        }

        if scope_set.contains(&RefreshableView::Remotes) {
            self.refresh_remotes();
        }

        if scope_set.contains(&RefreshableView::Worktrees) {
            self.refresh_worktrees();
        }

        if scope_set.contains(&RefreshableView::Staging) {
            self.refresh_staging();
        }

        if scope_set.contains(&RefreshableView::PatchBuilding) {
            self.refresh_patch_building();
        }

        if scope_set.contains(&RefreshableView::MergeConflicts)
            || scope_set.contains(&RefreshableView::Files)
        {
            self.refresh_merge_conflicts();
        }

        self.refresh_status();

        if let Some(callback) = options.then {
            callback();
        }
    }

    fn refresh_commits_and_branches(
        &self,
        _scope_set: &[RefreshableView],
        _options: &RefreshOptions,
    ) {
    }

    fn refresh_rebase_commits(&self) {}

    fn refresh_sub_commits_with_limit(&self) {}

    fn refresh_commit_files_context(&self) {}

    fn refresh_files_and_submodules(&self) {}

    fn refresh_stash_entries(&self) {}

    fn refresh_tags(&self) {}

    fn refresh_remotes(&self) {}

    fn refresh_worktrees(&self) {}

    fn refresh_staging(&self) {}

    fn refresh_patch_building(&self) {}

    fn refresh_merge_conflicts(&self) {}

    fn refresh_status(&self) {}

    fn refresh_view(&self, _context: &dyn Context) {}

    fn refresh_reflog_commits(&self) {}

    fn refresh_branches(
        &self,
        _refresh_worktrees: bool,
        _keep_branch_selection_index: bool,
        _load_behind_counts: bool,
    ) {
    }

    fn refresh_reflog_and_branches(
        &self,
        _refresh_worktrees: bool,
        _keep_branch_selection_index: bool,
    ) {
    }

    fn refresh_reflog_commits_considering_startup(&self) {}

    fn determine_checked_out_ref(&self) -> Option<Ref> {
        None
    }

    fn refresh_commits_with_limit(&self) -> Result<(), String> {
        Ok(())
    }

    fn refresh_authors(&self, _commits: &[Commit]) {}

    fn refresh_state_submodule_configs(&self) -> Result<(), String> {
        Ok(())
    }

    fn refresh_state_files(&self) -> Result<(), String> {
        Ok(())
    }

    fn load_worktrees(&self) {}

    fn ref_for_log(&self) -> String {
        "HEAD".to_string()
    }
}

pub trait Context {
    fn get_key(&self) -> &str;
}

pub struct Ref;
pub struct Commit;
pub struct Branch;

impl Ref {
    pub fn ref_name(&self) -> String {
        String::new()
    }
}

impl Default for RefreshOptions {
    fn default() -> Self {
        Self {
            mode: RefreshMode::Sync,
            scope: Vec::new(),
            then: None,
        }
    }
}

pub fn get_scope_names(scopes: &[RefreshableView]) -> Vec<String> {
    scopes
        .iter()
        .map(|s| format!("{:?}", s).to_lowercase())
        .collect()
}

pub fn get_mode_name(mode: &RefreshMode) -> String {
    match mode {
        RefreshMode::Sync => "sync".to_string(),
        RefreshMode::Async => "async".to_string(),
        RefreshMode::BlockUi => "block-ui".to_string(),
    }
}
