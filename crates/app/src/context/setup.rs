pub struct ContextTree {
    pub global: SimpleContext,
    pub status: SimpleContext,
    pub files: WorkingTreeContext,
    pub submodules: SubmodulesContext,
    pub menu: MenuContext,
    pub remotes: RemotesContext,
    pub worktrees: WorktreesContext,
    pub remote_branches: RemoteBranchesContext,
    pub local_commits: LocalCommitsContext,
    pub commit_files: CommitFilesContext,
    pub reflog_commits: ReflogCommitsContext,
    pub sub_commits: SubCommitsContext,
    pub branches: BranchesContext,
    pub tags: TagsContext,
    pub stash: StashContext,
    pub suggestions: SuggestionsContext,
    pub normal: MainContext,
    pub normal_secondary: MainContext,
    pub staging: PatchExplorerContext,
    pub staging_secondary: PatchExplorerContext,
    pub custom_patch_builder: PatchExplorerContext,
    pub custom_patch_builder_secondary: SimpleContext,
    pub merge_conflicts: MergeConflictsContext,
    pub confirmation: ConfirmationContext,
    pub prompt: PromptContext,
    pub commit_message: CommitMessageContext,
    pub commit_description: SimpleContext,
    pub search: SimpleContext,
    pub command_log: SimpleContext,
    pub snake: SimpleContext,
    pub options: DisplayContext,
    pub app_status: DisplayContext,
    pub search_prefix: DisplayContext,
    pub information: DisplayContext,
    pub limit: DisplayContext,
    pub status_spacer1: DisplayContext,
    pub status_spacer2: DisplayContext,
}

impl ContextTree {
    pub fn new() -> Self {
        Self {
            global: SimpleContext::new("GLOBAL_CONTEXT_KEY", false),
            status: SimpleContext::new("STATUS_CONTEXT_KEY", true),
            files: WorkingTreeContext::new(),
            submodules: SubmodulesContext::new(),
            menu: MenuContext::new(),
            remotes: RemotesContext::new(),
            worktrees: WorktreesContext::new(),
            remote_branches: RemoteBranchesContext::new(),
            local_commits: LocalCommitsContext::new(),
            commit_files: CommitFilesContext::new(),
            reflog_commits: ReflogCommitsContext::new(),
            sub_commits: SubCommitsContext::new(),
            branches: BranchesContext::new(),
            tags: TagsContext::new(),
            stash: StashContext::new(),
            suggestions: SuggestionsContext::new(),
            normal: MainContext::new("main", "NORMAL_MAIN_CONTEXT_KEY"),
            normal_secondary: MainContext::new("secondary", "NORMAL_SECONDARY_CONTEXT_KEY"),
            staging: PatchExplorerContext::new("STAGING_MAIN_CONTEXT_KEY"),
            staging_secondary: PatchExplorerContext::new("STAGING_SECONDARY_CONTEXT_KEY"),
            custom_patch_builder: PatchExplorerContext::new("PATCH_BUILDING_MAIN_CONTEXT_KEY"),
            custom_patch_builder_secondary: SimpleContext::new(
                "PATCH_BUILDING_SECONDARY_CONTEXT_KEY",
                false,
            ),
            merge_conflicts: MergeConflictsContext::new(),
            confirmation: ConfirmationContext::new(),
            prompt: PromptContext::new(),
            commit_message: CommitMessageContext::new(),
            commit_description: SimpleContext::new("COMMIT_DESCRIPTION_CONTEXT_KEY", true),
            search: SimpleContext::new("SEARCH_CONTEXT_KEY", true),
            command_log: SimpleContext::new("COMMAND_LOG_CONTEXT_KEY", true),
            snake: SimpleContext::new("SNAKE_CONTEXT_KEY", true),
            options: DisplayContext::new("OPTIONS_CONTEXT_KEY", "options"),
            app_status: DisplayContext::new("APP_STATUS_CONTEXT_KEY", "appStatus"),
            search_prefix: DisplayContext::new("SEARCH_PREFIX_CONTEXT_KEY", "searchPrefix"),
            information: DisplayContext::new("INFORMATION_CONTEXT_KEY", "information"),
            limit: DisplayContext::new("LIMIT_CONTEXT_KEY", "limit"),
            status_spacer1: DisplayContext::new("STATUS_SPACER1_CONTEXT_KEY", "statusSpacer1"),
            status_spacer2: DisplayContext::new("STATUS_SPACER2_CONTEXT_KEY", "statusSpacer2"),
        }
    }
}

impl Default for ContextTree {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SimpleContext {
    pub key: String,
    pub focusable: bool,
}

impl SimpleContext {
    pub fn new(key: &str, focusable: bool) -> Self {
        Self {
            key: key.to_string(),
            focusable,
        }
    }
}

pub struct WorkingTreeContext {
    pub key: String,
}

impl WorkingTreeContext {
    pub fn new() -> Self {
        Self {
            key: "FILES_CONTEXT_KEY".to_string(),
        }
    }
}

pub struct SubmodulesContext {
    pub key: String,
}

impl SubmodulesContext {
    pub fn new() -> Self {
        Self {
            key: "SUBMODULES_CONTEXT_KEY".to_string(),
        }
    }
}

pub struct MenuContext {
    pub key: String,
}

impl MenuContext {
    pub fn new() -> Self {
        Self {
            key: "MENU_CONTEXT_KEY".to_string(),
        }
    }
}

pub struct RemotesContext {
    pub key: String,
}

impl RemotesContext {
    pub fn new() -> Self {
        Self {
            key: "REMOTES_CONTEXT_KEY".to_string(),
        }
    }
}

pub struct WorktreesContext {
    pub key: String,
}

impl WorktreesContext {
    pub fn new() -> Self {
        Self {
            key: "WORKTREES_CONTEXT_KEY".to_string(),
        }
    }
}

pub struct RemoteBranchesContext {
    pub key: String,
}

impl RemoteBranchesContext {
    pub fn new() -> Self {
        Self {
            key: "REMOTE_BRANCHES_CONTEXT_KEY".to_string(),
        }
    }
}

pub struct LocalCommitsContext {
    pub key: String,
}

impl LocalCommitsContext {
    pub fn new() -> Self {
        Self {
            key: "LOCAL_COMMITS_CONTEXT_KEY".to_string(),
        }
    }
}

pub struct CommitFilesContext {
    pub key: String,
}

impl CommitFilesContext {
    pub fn new() -> Self {
        Self {
            key: "COMMIT_FILES_CONTEXT_KEY".to_string(),
        }
    }
}

pub struct ReflogCommitsContext {
    pub key: String,
}

impl ReflogCommitsContext {
    pub fn new() -> Self {
        Self {
            key: "REFLOG_COMMITS_CONTEXT_KEY".to_string(),
        }
    }
}

pub struct SubCommitsContext {
    pub key: String,
}

impl SubCommitsContext {
    pub fn new() -> Self {
        Self {
            key: "SUB_COMMITS_CONTEXT_KEY".to_string(),
        }
    }
}

pub struct BranchesContext {
    pub key: String,
}

impl BranchesContext {
    pub fn new() -> Self {
        Self {
            key: "BRANCHES_CONTEXT_KEY".to_string(),
        }
    }
}

pub struct TagsContext {
    pub key: String,
}

impl TagsContext {
    pub fn new() -> Self {
        Self {
            key: "TAGS_CONTEXT_KEY".to_string(),
        }
    }
}

pub struct StashContext {
    pub key: String,
}

impl StashContext {
    pub fn new() -> Self {
        Self {
            key: "STASH_CONTEXT_KEY".to_string(),
        }
    }
}

pub struct SuggestionsContext {
    pub key: String,
}

impl SuggestionsContext {
    pub fn new() -> Self {
        Self {
            key: "SUGGESTIONS_CONTEXT_KEY".to_string(),
        }
    }
}

pub struct MainContext {
    pub window_name: String,
    pub key: String,
}

impl MainContext {
    pub fn new(window_name: &str, key: &str) -> Self {
        Self {
            window_name: window_name.to_string(),
            key: key.to_string(),
        }
    }
}

pub struct PatchExplorerContext {
    pub key: String,
}

impl PatchExplorerContext {
    pub fn new(key: &str) -> Self {
        Self {
            key: key.to_string(),
        }
    }
}

pub struct MergeConflictsContext {
    pub key: String,
}

impl MergeConflictsContext {
    pub fn new() -> Self {
        Self {
            key: "MERGE_CONFLICTS_CONTEXT_KEY".to_string(),
        }
    }
}

pub struct ConfirmationContext {
    pub key: String,
}

impl ConfirmationContext {
    pub fn new() -> Self {
        Self {
            key: "CONFIRMATION_CONTEXT_KEY".to_string(),
        }
    }
}

pub struct PromptContext {
    pub key: String,
}

impl PromptContext {
    pub fn new() -> Self {
        Self {
            key: "PROMPT_CONTEXT_KEY".to_string(),
        }
    }
}

pub struct CommitMessageContext {
    pub key: String,
}

impl CommitMessageContext {
    pub fn new() -> Self {
        Self {
            key: "COMMIT_MESSAGE_CONTEXT_KEY".to_string(),
        }
    }
}

pub struct DisplayContext {
    pub key: String,
    pub window_name: String,
}

impl DisplayContext {
    pub fn new(key: &str, window_name: &str) -> Self {
        Self {
            key: key.to_string(),
            window_name: window_name.to_string(),
        }
    }
}
