use crate::GitResult;
use crate::RepoPaths;

pub struct StatusCommands {
    repo_paths: RepoPaths,
}

impl StatusCommands {
    pub fn new(repo_paths: RepoPaths) -> Self {
        Self { repo_paths }
    }

    pub fn working_tree_state(&self) -> WorkingTreeState {
        let mut state = WorkingTreeState::default();
        state.rebasing = self.is_in_rebase().unwrap_or(false);
        state.merging = self.is_in_merge_state().unwrap_or(false);
        state.cherry_picking = self.is_in_cherry_pick().unwrap_or(false);
        state.reverting = self.is_in_revert().unwrap_or(false);
        state
    }

    pub fn is_bare_repo(&self) -> bool {
        self.repo_paths.is_bare_repo()
    }

    pub fn is_in_rebase(&self) -> GitResult<bool> {
        let git_dir = self.repo_paths.worktree_git_dir_path();
        let rebase_merge_path = git_dir.join("rebase-merge");

        if rebase_merge_path.exists() {
            return Ok(true);
        }

        let rebase_apply_path = git_dir.join("rebase-apply");
        Ok(rebase_apply_path.exists())
    }

    pub fn is_in_merge_state(&self) -> GitResult<bool> {
        let git_dir = self.repo_paths.worktree_git_dir_path();
        let merge_head_path = git_dir.join("MERGE_HEAD");
        Ok(merge_head_path.exists())
    }

    pub fn is_in_cherry_pick(&self) -> GitResult<bool> {
        let git_dir = self.repo_paths.worktree_git_dir_path();
        let cherry_pick_head_path = git_dir.join("CHERRY_PICK_HEAD");

        if !cherry_pick_head_path.exists() {
            return Ok(false);
        }

        let stopped_sha_path = git_dir.join("rebase-merge").join("stopped-sha");
        if !stopped_sha_path.exists() {
            return Ok(true);
        }

        let cherry_pick_head = match std::fs::read(cherry_pick_head_path) {
            Ok(c) => c,
            Err(_) => return Ok(true),
        };
        let stopped_sha = match std::fs::read(&stopped_sha_path) {
            Ok(s) => s,
            Err(_) => return Ok(true),
        };

        let cherry_pick_head_str = String::from_utf8_lossy(&cherry_pick_head)
            .trim()
            .to_string();
        let stopped_sha_str = String::from_utf8_lossy(&stopped_sha).trim().to_string();

        Ok(!cherry_pick_head_str.starts_with(&stopped_sha_str))
    }

    pub fn is_in_revert(&self) -> GitResult<bool> {
        let git_dir = self.repo_paths.worktree_git_dir_path();
        let revert_head_path = git_dir.join("REVERT_HEAD");
        Ok(revert_head_path.exists())
    }

    pub fn branch_being_rebased(&self) -> GitResult<String> {
        for dir in &["rebase-merge", "rebase-apply"] {
            let git_dir = self.repo_paths.worktree_git_dir_path();
            let head_name_path = git_dir.join(dir).join("head-name");
            if head_name_path.exists() {
                let bytes_content = match std::fs::read(&head_name_path) {
                    Ok(b) => b,
                    Err(_) => continue,
                };
                return Ok(String::from_utf8_lossy(&bytes_content).trim().to_string());
            }
        }
        Ok(String::new())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkingTreeState {
    pub rebasing: bool,
    pub merging: bool,
    pub cherry_picking: bool,
    pub reverting: bool,
}
