pub trait ControllerInitializer {
    fn reset_controllers(&mut self);
}

pub struct GuiControllers {
    pub helpers: GuiHelpers,
}

pub struct GuiHelpers {
    pub refs: (),
    pub host: (),
    pub patch_building: (),
    pub staging: (),
    pub bisect: (),
    pub suggestions: (),
    pub files: (),
    pub working_tree: (),
    pub tags: (),
    pub branches_helper: (),
    pub gpg: (),
    pub merge_and_rebase: (),
    pub merge_conflicts: (),
    pub cherry_pick: (),
    pub upstream: (),
    pub amend_helper: (),
    pub fixup_helper: (),
    pub commits: (),
    pub suspend_resume: (),
    pub snake: (),
    pub diff: (),
    pub repos: (),
    pub record_directory: (),
    pub update: (),
    pub window: (),
    pub view: (),
    pub refresh: (),
    pub confirmation: (),
    pub mode: (),
    pub app_status: (),
    pub inline_status: (),
    pub window_arrangement: (),
    pub search: (),
    pub worktree: (),
    pub sub_commits: (),
}

impl GuiHelpers {
    pub fn new() -> Self {
        Self {
            refs: (),
            host: (),
            patch_building: (),
            staging: (),
            bisect: (),
            suggestions: (),
            files: (),
            working_tree: (),
            tags: (),
            branches_helper: (),
            gpg: (),
            merge_and_rebase: (),
            merge_conflicts: (),
            cherry_pick: (),
            upstream: (),
            amend_helper: (),
            fixup_helper: (),
            commits: (),
            suspend_resume: (),
            snake: (),
            diff: (),
            repos: (),
            record_directory: (),
            update: (),
            window: (),
            view: (),
            refresh: (),
            confirmation: (),
            mode: (),
            app_status: (),
            inline_status: (),
            window_arrangement: (),
            search: (),
            worktree: (),
            sub_commits: (),
        }
    }
}

impl Default for GuiHelpers {
    fn default() -> Self {
        Self::new()
    }
}

impl GuiControllers {
    pub fn new() -> Self {
        Self {
            helpers: GuiHelpers::new(),
        }
    }
}

impl Default for GuiControllers {
    fn default() -> Self {
        Self::new()
    }
}

impl ControllerInitializer for GuiControllers {
    fn reset_controllers(&mut self) {
        // Reset all controllers
    }
}
