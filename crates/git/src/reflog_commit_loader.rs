use super_lazygit_core::state::{CommitItem, CommitStatus, CommitTodoAction};

use crate::{git_builder_output, GitCommandBuilder, GitResult};

pub struct ReflogCommitLoader {
    repo_path: std::path::PathBuf,
}

impl ReflogCommitLoader {
    pub fn new(repo_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
        }
    }

    /// Get reflog commits, optionally filtered
    /// Returns (commits, only_obtained_new_reflog_commits, error)
    pub fn get_reflog_commits(
        &self,
        last_reflog_commit: Option<&CommitItem>,
        filter_path: Option<&str>,
        filter_author: Option<&str>,
    ) -> GitResult<(Vec<CommitItem>, bool)> {
        let mut cmd = GitCommandBuilder::new("log")
            .config("log.showSignature=false")
            .arg(["-g", "--format=+%H%x00%ct%x00%gs%x00%P"]);

        if let Some(author) = filter_author {
            cmd = cmd.arg([format!("--author={}", author)]);
        }

        if let Some(path) = filter_path {
            cmd = cmd.arg(["--follow", "--name-status", "--", path]);
        }

        let output = git_builder_output(&self.repo_path, cmd)?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        let mut commits = Vec::new();
        let mut only_obtained_new = false;

        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some(commit) = self.parse_line(line) {
                // Check if we've reached the last known reflog commit
                if let Some(last) = last_reflog_commit {
                    if self.same_reflog_commit(&commit, last) {
                        only_obtained_new = true;
                        // Stop here - we've reached the old commits
                        break;
                    }
                }

                commits.push(commit);
            }
        }

        Ok((commits, only_obtained_new))
    }

    fn same_reflog_commit(&self, a: &CommitItem, b: &CommitItem) -> bool {
        a.oid == b.oid && a.unix_timestamp == b.unix_timestamp && a.summary == b.summary
    }

    fn parse_line(&self, line: &str) -> Option<CommitItem> {
        let fields: Vec<&str> = line.splitn(4, '\x00').collect();

        if fields.len() < 4 {
            return None;
        }

        let hash = fields[0];
        let unix_timestamp: i64 = fields[1].parse().unwrap_or(0);
        let message = fields[2].to_string();
        let parent_hashes = fields[3];

        let parents: Vec<String> = if parent_hashes.is_empty() {
            Vec::new()
        } else {
            parent_hashes.split(' ').map(String::from).collect()
        };

        let commit = CommitItem {
            oid: hash.to_string(),
            short_oid: hash[..7.min(hash.len())].to_string(),
            summary: message,
            tags: Vec::new(),
            extra_info: String::new(),
            author_name: String::new(),
            author_email: String::new(),
            unix_timestamp,
            parents,
            status: CommitStatus::Reflog,
            todo_action: CommitTodoAction::None,
            todo_action_flag: String::new(),
            divergence: Default::default(),
            filter_paths: Vec::new(),
            changed_files: Vec::new(),
            diff: Default::default(),
        };

        Some(commit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_line() {
        let loader = ReflogCommitLoader::new("/tmp");
        let line = "abc123\x001700000000\x00update: some commit\x00parent1 parent2";

        let result = loader.parse_line(line);
        assert!(result.is_some());

        let commit = result.unwrap();
        assert_eq!(commit.oid, "abc123");
        assert_eq!(commit.short_oid, "abc123");
        assert_eq!(commit.unix_timestamp, 1700000000);
        assert_eq!(commit.summary, "update: some commit");
        assert_eq!(commit.status, CommitStatus::Reflog);
        assert_eq!(commit.parents, vec!["parent1", "parent2"]);
    }

    #[test]
    fn test_parse_line_invalid() {
        let loader = ReflogCommitLoader::new("/tmp");
        let line = "only one field";

        assert!(loader.parse_line(line).is_none());
    }

    #[test]
    fn test_same_reflog_commit() {
        let loader = ReflogCommitLoader::new("/tmp");

        let commit1 = CommitItem {
            oid: "abc123".to_string(),
            short_oid: "abc123".to_string(),
            summary: "test commit".to_string(),
            unix_timestamp: 1700000000,
            ..Default::default()
        };

        let commit2 = CommitItem {
            oid: "abc123".to_string(),
            short_oid: "abc123".to_string(),
            summary: "test commit".to_string(),
            unix_timestamp: 1700000000,
            ..Default::default()
        };

        let commit3 = CommitItem {
            oid: "different".to_string(),
            short_oid: "diff".to_string(),
            summary: "test commit".to_string(),
            unix_timestamp: 1700000000,
            ..Default::default()
        };

        assert!(loader.same_reflog_commit(&commit1, &commit2));
        assert!(!loader.same_reflog_commit(&commit1, &commit3));
    }
}
