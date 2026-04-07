use std::path::PathBuf;

use super_lazygit_core::state::{CommitFileItem, FileStatusKind};

use crate::GitCommandBuilder;
use crate::GitResult;

/// Retrieves commit files (files changed in a commit or commit range)
pub struct CommitFileLoader {
    repo_path: PathBuf,
}

impl CommitFileLoader {
    pub fn new(repo_path: PathBuf) -> Self {
        Self { repo_path }
    }

    /// GetFilesInDiff gets the specified commit files
    pub fn get_files_in_diff(
        &self,
        from: &str,
        to: &str,
        reverse: bool,
    ) -> GitResult<Vec<CommitFileItem>> {
        let mut builder = GitCommandBuilder::new("diff")
            .config("diff.noprefix=false")
            .arg(["--submodule", "--no-ext-diff", "--name-status", "-z", "--no-renames"]);

        if reverse {
            builder = builder.arg(["-R"]);
        }

        builder = builder.arg([from, to]);

        let output = git_builder_output(&self.repo_path, builder)?;

        if !output.status.success() {
            return Err(crate::GitError::OperationFailed {
                message: format!(
                    "git diff failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }

        let filenames = String::from_utf8_lossy(&output.stdout);
        Ok(get_commit_files_from_filenames(&filenames))
    }
}

/// Parses the filenames string (something like "MM\x00file1\x00MU\x00file2\x00AA\x00file3\x00")
/// Split by null character and map each status-name pair to a commit file
fn get_commit_files_from_filenames(filenames: &str) -> Vec<CommitFileItem> {
    let trimmed = filenames.trim_end_matches('\x00');
    if trimmed.is_empty() {
        return Vec::new();
    }

    let lines: Vec<&str> = trimmed.split('\x00').collect();
    if lines.len() < 2 {
        return Vec::new();
    }

    // Chunk into pairs (status, filename)
    let mut results = Vec::new();
    for chunk in lines.chunks(2) {
        if chunk.len() == 2 {
            results.push(CommitFileItem {
                path: PathBuf::from(chunk[1]),
                kind: kind_from_change_status(chunk[0]),
            });
        }
    }

    results
}

/// Maps a change status string (like "A", "M", "D", etc.) to FileStatusKind
fn kind_from_change_status(status: &str) -> FileStatusKind {
    match status {
        "A" => FileStatusKind::Added,
        "M" => FileStatusKind::Modified,
        "D" => FileStatusKind::Deleted,
        "R" => FileStatusKind::Renamed,
        "C" => FileStatusKind::Renamed, // Copy treated as Renamed for status purposes
        "U" => FileStatusKind::Conflicted,
        _ => FileStatusKind::Modified,
    }
}

fn git_builder_output(
    repo_path: &PathBuf,
    builder: GitCommandBuilder,
) -> GitResult<std::process::Output> {
    use std::process::Command;

    let argv = builder.into_args();
    let mut cmd = Command::new("git");
    cmd.current_dir(repo_path).args(&argv);

    cmd.output()
        .map_err(|e| crate::GitError::OperationFailed {
            message: format!("failed to execute git: {}", e),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_commit_files_from_filenames_empty() {
        assert!(get_commit_files_from_filenames("").is_empty());
        assert!(get_commit_files_from_filenames("\x00").is_empty());
    }

    #[test]
    fn test_get_commit_files_from_filenames_single() {
        // Only one element (status without filename) should return empty
        let result = get_commit_files_from_filenames("A");
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_commit_files_from_filenames_pairs() {
        // "A\x00file1\x00M\x00file2" -> [("A", "file1"), ("M", "file2")]
        let result = get_commit_files_from_filenames("A\x00file1\x00M\x00file2");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].path, PathBuf::from("file1"));
        assert!(matches!(result[0].kind, FileStatusKind::Added));
        assert_eq!(result[1].path, PathBuf::from("file2"));
        assert!(matches!(result[1].kind, FileStatusKind::Modified));
    }

    #[test]
    fn test_kind_from_change_status() {
        assert!(matches!(kind_from_change_status("A"), FileStatusKind::Added));
        assert!(matches!(kind_from_change_status("M"), FileStatusKind::Modified));
        assert!(matches!(kind_from_change_status("D"), FileStatusKind::Deleted));
        assert!(matches!(kind_from_change_status("R"), FileStatusKind::Renamed));
        assert!(matches!(kind_from_change_status("C"), FileStatusKind::Renamed));
        assert!(matches!(kind_from_change_status("U"), FileStatusKind::Conflicted));
        assert!(matches!(kind_from_change_status("X"), FileStatusKind::Modified)); // Unknown defaults to Modified
    }
}
