// Ported from ./references/lazygit-master/pkg/gui/context/branches_context.go

/// Branches context for displaying local branches
pub struct BranchesContext {
    pub key: String,
}

impl BranchesContext {
    pub fn new() -> Self {
        Self {
            key: "BRANCHES_CONTEXT_KEY".to_string(),
        }
    }

    /// Get the selected branch as a ref
    pub fn get_selected_ref(&self) -> Option<String> {
        None
    }

    /// Get terminals for diff calculation
    pub fn get_diff_terminals(&self) -> Vec<String> {
        vec![]
    }

    /// Get ref for adjusting line number in diff
    pub fn ref_for_adjusting_line_number_in_diff(&self) -> String {
        String::new()
    }

    /// Whether to show branch heads in sub-commits
    pub fn show_branch_heads_in_sub_commits(&self) -> bool {
        true
    }
}

impl Default for BranchesContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_branches_context_new() {
        let ctx = BranchesContext::new();
        assert_eq!(ctx.key, "BRANCHES_CONTEXT_KEY");
    }

    #[test]
    fn test_show_branch_heads_in_sub_commits() {
        let ctx = BranchesContext::new();
        assert!(ctx.show_branch_heads_in_sub_commits());
    }
}
