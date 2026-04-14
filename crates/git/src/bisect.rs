use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use crate::bisect_info::{get_info_for_git_dir, BisectInfo, BisectStatus};
use crate::{git, git_stdout, GitError, GitResult};

pub struct BisectCommands;

impl BisectCommands {
    pub fn new() -> Self {
        Self
    }

    pub fn get_info(&self, repo_paths: &crate::RepoPaths) -> BisectInfo {
        self.get_info_for_git_dir(repo_paths.worktree_git_dir_path())
    }

    pub fn get_info_for_git_dir(&self, git_dir: &Path) -> BisectInfo {
        get_info_for_git_dir(git_dir)
    }

    pub fn reset(&self, repo_path: &Path) -> GitResult<()> {
        git(repo_path, ["bisect", "reset"])
    }

    pub fn mark(&self, repo_path: &Path, r#ref: &str, term: &str) -> GitResult<()> {
        git(repo_path, ["bisect", term, r#ref])
    }

    pub fn skip(&self, repo_path: &Path, r#ref: &str) -> GitResult<()> {
        self.mark(repo_path, r#ref, "skip")
    }

    pub fn start(&self, repo_path: &Path) -> GitResult<()> {
        git(repo_path, ["bisect", "start"])
    }

    pub fn start_with_terms(
        &self,
        repo_path: &Path,
        old_term: &str,
        new_term: &str,
    ) -> GitResult<()> {
        git(
            repo_path,
            [
                "bisect",
                "start",
                "--term-old",
                old_term,
                "--term-new",
                new_term,
            ],
        )
    }

    pub fn is_done(&self, repo_paths: &crate::RepoPaths) -> (bool, Vec<String>, Option<GitError>) {
        let info = self.get_info(repo_paths);
        if !info.bisecting() {
            return (false, Vec::new(), None);
        }

        let new_hash = info.get_new_hash();
        if new_hash.is_empty() {
            return (false, Vec::new(), None);
        }

        let mut done = false;
        let mut candidates: Vec<String> = Vec::new();
        let status_map = info.status_map().clone();

        let output = match Command::new("git")
            .args(["rev-list", &new_hash])
            .current_dir(repo_paths.repo_path())
            .output()
        {
            Ok(output) => output,
            Err(e) => {
                return (
                    false,
                    Vec::new(),
                    Some(GitError::OperationFailed {
                        message: e.to_string(),
                    }),
                )
            }
        };

        if !output.status.success() {
            return (
                false,
                Vec::new(),
                Some(GitError::OperationFailed {
                    message: "rev-list command failed".to_string(),
                }),
            );
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let hash = line.trim().to_string();

            if let Some(&status) = status_map.get(&hash) {
                match status {
                    BisectStatus::Skipped | BisectStatus::New => {
                        candidates.push(hash);
                    }
                    BisectStatus::Old => {
                        done = true;
                        break;
                    }
                }
            } else {
                break;
            }
        }

        (done, candidates, None)
    }

    pub fn reachable_from_start(&self, repo_path: &Path, bisect_info: &BisectInfo) -> bool {
        let new_hash = bisect_info.get_new_hash();
        let start_hash = bisect_info.get_start_hash();

        if new_hash.is_empty() || start_hash.is_empty() {
            return false;
        }

        git(
            repo_path,
            ["merge-base", "--is-ancestor", &new_hash, &start_hash],
        )
        .is_ok()
    }
}

impl Default for BisectCommands {
    fn default() -> Self {
        Self::new()
    }
}
