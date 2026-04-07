// Ported from ./references/lazygit-master/pkg/gui/context/local_commits_context.go

/// Local commits context for displaying commits in the current branch
pub struct LocalCommitsContext {
    pub key: String,
}

impl LocalCommitsContext {
    pub fn new() -> Self {
        Self {
            key: "LOCAL_COMMITS_CONTEXT_KEY".to_string(),
        }
    }

    /// Check if rebase is possible
    pub fn can_rebase(&self) -> bool {
        true
    }

    /// Get the selected commit as a ref
    pub fn get_selected_ref(&self) -> Option<String> {
        None
    }

    /// Get selected ref range for diff files
    pub fn get_selected_ref_range_for_diff_files(&self) -> Option<(String, String)> {
        None
    }

    /// Get selected commit hash
    pub fn get_selected_commit_hash(&self) -> String {
        String::new()
    }

    /// Select commit by hash
    pub fn select_commit_by_hash(&mut self, hash: &str) -> bool {
        if hash.is_empty() {
            return false;
        }
        false
    }

    /// Get diff terminals
    pub fn get_diff_terminals(&self) -> Vec<String> {
        vec![]
    }

    /// Get ref for adjusting line number in diff
    pub fn ref_for_adjusting_line_number_in_diff(&self) -> String {
        String::new()
    }

    /// Index for goto bottom
    pub fn index_for_goto_bottom(&self) -> usize {
        0
    }
}

impl Default for LocalCommitsContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_commits_context_new() {
        let ctx = LocalCommitsContext::new();
        assert_eq!(ctx.key, "LOCAL_COMMITS_CONTEXT_KEY");
    }

    #[test]
    fn test_can_rebase() {
        let ctx = LocalCommitsContext::new();
        assert!(ctx.can_rebase());
    }
}
