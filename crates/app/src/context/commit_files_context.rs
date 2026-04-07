// Ported from ./references/lazygit-master/pkg/gui/context/commit_files_context.go

/// Commit files context for displaying files in a commit
pub struct CommitFilesContext {
    pub key: String,
    title_ref: String,
}

impl CommitFilesContext {
    pub fn new() -> Self {
        Self {
            key: "COMMIT_FILES_CONTEXT_KEY".to_string(),
            title_ref: String::new(),
        }
    }

    /// Get terminals for diff calculation
    pub fn get_diff_terminals(&self) -> Vec<String> {
        vec![]
    }

    /// Get ref for adjusting line number in diff
    pub fn ref_for_adjusting_line_number_in_diff(&self) -> String {
        String::new()
    }

    /// Get from and to refs for diff
    pub fn get_from_and_to_for_diff(&self) -> (String, String) {
        (String::new(), String::new())
    }

    /// Re-initialize with new ref and ref range
    pub fn re_init(&mut self, _ref: &str, _ref_range: Option<(String, String)>) {
        self.title_ref = String::new();
    }

    /// Get the title ref
    pub fn title(&self) -> &str {
        &self.title_ref
    }
}

impl Default for CommitFilesContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commit_files_context_new() {
        let ctx = CommitFilesContext::new();
        assert_eq!(ctx.key, "COMMIT_FILES_CONTEXT_KEY");
    }
}
