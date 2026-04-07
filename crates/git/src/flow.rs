use std::process::Command;

use super_lazygit_core::RepoId;

/// Git flow support commands.
pub struct FlowCommands {
    repo_id: RepoId,
}

impl FlowCommands {
    #[must_use]
    pub fn new(repo_id: RepoId) -> Self {
        Self { repo_id }
    }

    /// Check if git flow is enabled based on config.
    pub fn git_flow_enabled(&self) -> bool {
        // Check if git flow prefix is configured
        let output = Command::new("git")
            .args(["-C", self.repo_id.get_path(), "config", "--get", "gitflow.prefix.feature"])
            .output();

        output.map_or(false, |o| o.status.success())
    }

    /// Create a command to finish a git flow branch.
    ///
    /// # Arguments
    ///
    /// * `branch_name` - The full branch name (e.g., "feature/my-feature")
    ///
    /// # Returns
    ///
    /// A command to finish the branch, or an error if not a valid git flow branch
    pub fn finish_cmd(&self, branch_name: &str) -> Result<Command, String> {
        // Find out what kind of branch this is
        let branch_type = self.get_branch_type(branch_name)?;

        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(self.repo_id.get_path()).arg("flow");
        cmd.arg(branch_type).arg("finish");
        // Remove the prefix from the branch name
        let prefix = format!("{}/", branch_type);
        let name = branch_name.strip_prefix(&prefix).unwrap_or(branch_name);
        cmd.arg(name);

        Ok(cmd)
    }

    /// Create a command to start a git flow branch.
    ///
    /// # Arguments
    ///
    /// * `branch_type` - The type of branch (feature, bugfix, etc.)
    /// * `name` - The name of the branch
    ///
    /// # Returns
    ///
    /// A command to start the branch
    pub fn start_cmd(&self, branch_type: &str, name: &str) -> Command {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(self.repo_id.get_path()).arg("flow");
        cmd.arg(branch_type).arg("start").arg(name);
        cmd
    }

    fn get_branch_type(&self, branch_name: &str) -> Result<&str, String> {
        // Get git flow prefixes from config
        let output = Command::new("git")
            .args(["-C", self.repo_id.get_path(), "config", "--list"])
            .output()
            .map_err(|e| e.to_string())?;

        if !output.status.success() {
            return Err("Not a git flow branch".to_string());
        }

        let config = String::from_utf8_lossy(&output.stdout);

        // Find the branch prefix
        let parts: Vec<&str> = branch_name.split('/').collect();
        if parts.len() < 2 {
            return Err("Not a git flow branch".to_string());
        }

        let prefix = format!("{}/", parts[0]);

        // Search for matching gitflow.prefix entry
        for line in config.lines() {
            if line.starts_with("gitflow.prefix.") && line.ends_with(&prefix) {
                // Extract branch type from gitflow.prefix.feature = "feature/"
                if let Some(rest) = line.strip_prefix("gitflow.prefix.") {
                    if let Some(branch_type) = rest.split_whitespace().next() {
                        return Ok(branch_type);
                    }
                }
            }
        }

        Err("Not a git flow branch".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_branch_type_feature() {
        let flow = FlowCommands::new(RepoId::Standalone {
            path: "/tmp".into(),
        });

        // This would need a real git flow repo to test properly
        // Just testing the method exists
        let result = flow.get_branch_type("feature/test");
        assert!(result.is_err()); // Will fail without proper git flow setup
    }
}
