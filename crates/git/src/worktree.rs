use super_lazygit_core::state::WorktreeItem;
use super_lazygit_core::BranchItem;

pub fn worktree_for_branch<'a>(
    branch: &BranchItem,
    worktrees: &'a [WorktreeItem],
) -> Option<&'a WorktreeItem> {
    worktrees
        .iter()
        .find(|worktree| worktree.branch.as_ref() == Some(&branch.name))
}

pub fn checked_out_by_other_worktree(branch: &BranchItem, worktrees: &[WorktreeItem]) -> bool {
    let Some(worktree) = worktree_for_branch(branch, worktrees) else {
        return false;
    };
    !worktree.is_current
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_branch(name: &str) -> BranchItem {
        BranchItem {
            name: name.to_string(),
            ..Default::default()
        }
    }

    fn make_worktree(path: &str, branch: Option<&str>, is_current: bool) -> WorktreeItem {
        WorktreeItem {
            path: PathBuf::from(path),
            branch: branch.map(String::from),
            is_current,
            ..Default::default()
        }
    }

    #[test]
    fn test_worktree_for_branch_found() {
        let branch = make_branch("feature");
        let worktrees = vec![
            make_worktree("/repo", Some("main"), true),
            make_worktree("/repo/feature", Some("feature"), false),
        ];
        assert_eq!(
            worktree_for_branch(&branch, &worktrees).map(|w| w.path.to_str().unwrap()),
            Some("/repo/feature")
        );
    }

    #[test]
    fn test_worktree_for_branch_not_found() {
        let branch = make_branch("nonexistent");
        let worktrees = vec![make_worktree("/repo", Some("main"), true)];
        assert!(worktree_for_branch(&branch, &worktrees).is_none());
    }

    #[test]
    fn test_checked_out_by_other_worktree_true() {
        let branch = make_branch("feature");
        let worktrees = vec![
            make_worktree("/repo", Some("main"), true),
            make_worktree("/repo/feature", Some("feature"), false),
        ];
        assert!(checked_out_by_other_worktree(&branch, &worktrees));
    }

    #[test]
    fn test_checked_out_by_other_worktree_false_current() {
        let branch = make_branch("feature");
        let worktrees = vec![make_worktree("/repo/feature", Some("feature"), true)];
        assert!(!checked_out_by_other_worktree(&branch, &worktrees));
    }

    #[test]
    fn test_checked_out_by_other_worktree_no_worktree() {
        let branch = make_branch("nonexistent");
        let worktrees = vec![make_worktree("/repo", Some("main"), true)];
        assert!(!checked_out_by_other_worktree(&branch, &worktrees));
    }
}
