use std::ffi::OsString;
use std::process::Command;

use super_lazygit_core::RepoId;

/// Git blame functionality for a file.
pub struct BlameCommands {
    repo_id: RepoId,
}

impl BlameCommands {
    #[must_use]
    pub fn new(repo_id: RepoId) -> Self {
        Self { repo_id }
    }

    /// Blame a range of lines. For each line, output the hash of the commit where
    /// the line last changed, then a space, then a description of the commit (author
    /// and date), another space, and then the line.
    ///
    /// # Arguments
    ///
    /// * `filename` - The file to blame
    /// * `commit` - The commit to blame from
    /// * `first_line` - The first line of the range
    /// * `num_lines` - The number of lines to blame
    ///
    /// # Returns
    ///
    /// The blame output as a string, or an error
    pub fn blame_line_range(
        &self,
        filename: &str,
        commit: &str,
        first_line: usize,
        num_lines: usize,
    ) -> Result<String, std::io::Error> {
        let output = Command::new("git")
            .args([
                "-C",
                self.repo_id.get_path(),
                "blame",
                "-l",
                &format!("-L{first_line},+{num_lines}"),
                commit,
                "--",
                filename,
            ])
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                String::from_utf8_lossy(&output.stderr),
            ))
        }
    }
}
