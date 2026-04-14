use std::path::PathBuf;
use std::process::Command;

use super_lazygit_core::RepoId;

use crate::branch_loader::BranchInfo;
use crate::{git_builder_output, GitCommandBuilder, GitResult};

pub struct BranchCommands {
    repo_id: RepoId,
    all_branches_log_cmd_index: usize,
}

impl BranchCommands {
    #[must_use]
    pub fn new(repo_id: RepoId) -> Self {
        Self {
            repo_id,
            all_branches_log_cmd_index: 0,
        }
    }

    pub fn current_branch_info(&self) -> GitResult<BranchInfo> {
        let repo_path = PathBuf::from(&self.repo_id.0);

        let symbolic_ref_output = git_builder_output(
            &repo_path,
            GitCommandBuilder::new("symbolic-ref").arg(["--short", "HEAD"]),
        );

        if let Ok(output) = symbolic_ref_output {
            if output.status.success() {
                let branch_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !branch_name.is_empty() && branch_name != "HEAD" {
                    return Ok(BranchInfo {
                        ref_name: branch_name.clone(),
                        display_name: branch_name,
                        detached_head: false,
                    });
                }
            }
        }

        let branch_output = git_builder_output(
            &repo_path,
            GitCommandBuilder::new("branch").arg([
                "--points-at=HEAD",
                "--format=%(HEAD)%00%(objectname)%00%(refname)",
            ]),
        )?;

        for line in String::from_utf8_lossy(&branch_output.stdout).lines() {
            let parts: Vec<&str> = line.trim_end_matches(['\r', '\n']).split('\x00').collect();
            if parts.len() == 3 && parts[0] == "*" {
                let hash = parts[1].to_string();
                let display_name = parts[2].to_string();
                return Ok(BranchInfo {
                    ref_name: hash,
                    display_name,
                    detached_head: true,
                });
            }
        }

        Ok(BranchInfo {
            ref_name: "HEAD".to_string(),
            display_name: "HEAD".to_string(),
            detached_head: true,
        })
    }

    pub fn current_branch_name(&self) -> GitResult<String> {
        let repo_path = PathBuf::from(&self.repo_id.0);
        let output = git_builder_output(
            &repo_path,
            GitCommandBuilder::new("branch").arg(["--show-current"]),
        )?;

        if !output.status.success() {
            return Err(crate::GitError::OperationFailed {
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn get_upstream_difference_count(&self, branch_name: &str) -> (String, String) {
        let (pushable, pullable) =
            self.get_commit_differences(branch_name, &format!("{branch_name}@{{u}}"));
        (
            pushable.unwrap_or_else(|_| "?".to_string()),
            pullable.unwrap_or_else(|_| "?".to_string()),
        )
    }

    fn get_commit_differences(
        &self,
        from: &str,
        to: &str,
    ) -> (GitResult<String>, GitResult<String>) {
        let repo_path = PathBuf::from(&self.repo_id.0);
        let pushable = self.count_differences(&repo_path, to, from);
        let pullable = self.count_differences(&repo_path, from, to);
        (pushable, pullable)
    }

    fn count_differences(&self, repo_path: &PathBuf, from: &str, to: &str) -> GitResult<String> {
        let output = git_builder_output(
            repo_path,
            GitCommandBuilder::new("rev-list")
                .arg([format!("{from}..{to}"), "--count".to_string()]),
        )?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn all_branches_log_cmd_index(&self) -> usize {
        self.all_branches_log_cmd_index
    }

    pub fn rotate_all_branches_log_idx(&mut self) {
        self.all_branches_log_cmd_index =
            (self.all_branches_log_cmd_index + 1) % 1.max(self.all_branches_log_cmd_index);
    }

    pub fn rotate_all_branches_log_idx_backward(&mut self) {
        let n = 1.max(self.all_branches_log_cmd_index);
        self.all_branches_log_cmd_index =
            (self.all_branches_log_cmd_index.saturating_sub(1) + n) % n;
    }

    pub fn get_all_branches_log_idx_and_count(&self) -> (usize, usize) {
        let n = 1.max(self.all_branches_log_cmd_index);
        (self.all_branches_log_cmd_index, n)
    }

    pub fn is_branch_merged(&self, branch_name: &str) -> GitResult<bool> {
        let repo_path = PathBuf::from(&self.repo_id.0);

        let upstream = format!("{branch_name}@{{upstream}}");
        let has_upstream = Command::new("git")
            .arg("-C")
            .arg(&self.repo_id.0)
            .arg("rev-parse")
            .arg("--symbolic-full-name")
            .arg(&upstream)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        let mut branches_to_check = vec!["HEAD".to_string()];
        if has_upstream {
            branches_to_check.push(upstream);
        }

        branches_to_check.extend(vec!["main".to_string(), "master".to_string()]);

        let mut args = vec![
            "rev-list".to_string(),
            "--max-count=1".to_string(),
            branch_name.to_string(),
        ];
        for b in &branches_to_check {
            args.push(format!("^{b}"));
        }
        args.push("--".to_string());

        let output = git_builder_output(&repo_path, GitCommandBuilder::new("rev-list").arg(args))?;

        Ok(String::from_utf8_lossy(&output.stdout).trim().is_empty())
    }
}
