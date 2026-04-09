use std::path::PathBuf;
use std::process::Command;

use super_lazygit_core::MergeVariant;

use crate::{git, git_builder, git_builder_output, GitCommandBuilder, GitResult, RepoId};

pub struct BranchCommands {
    repo_id: RepoId,
}

impl BranchCommands {
    #[must_use]
    pub fn new(repo_id: RepoId) -> Self {
        Self { repo_id }
    }

    pub fn new_branch(&self, name: &str, base: &str) -> GitResult<()> {
        git(
            PathBuf::from(&self.repo_id.0).as_path(),
            ["checkout", "-b", name, base],
        )
    }

    pub fn new_branch_without_tracking(&self, name: &str, base: &str) -> GitResult<()> {
        git(
            PathBuf::from(&self.repo_id.0).as_path(),
            ["checkout", "-b", name, base, "--no-track"],
        )
    }

    pub fn new_branch_without_checkout(&self, name: &str, base: &str) -> GitResult<()> {
        git(
            PathBuf::from(&self.repo_id.0).as_path(),
            ["branch", name, base],
        )
    }

    pub fn create_with_upstream(&self, name: &str, upstream: &str) -> GitResult<()> {
        git(
            PathBuf::from(&self.repo_id.0).as_path(),
            ["branch", "--track", name, upstream],
        )
    }

    pub fn local_delete(&self, branches: &[&str], force: bool) -> GitResult<()> {
        let mut args = vec!["branch".to_string()];
        if force {
            args.push("-D".to_string());
        } else {
            args.push("-d".to_string());
        }
        for branch in branches {
            args.push(branch.to_string());
        }
        git(PathBuf::from(&self.repo_id.0).as_path(), &args)
    }

    pub fn checkout(&self, branch: &str, force: bool, env_vars: &[(&str, &str)]) -> GitResult<()> {
        let mut cmd = GitCommandBuilder::new("checkout");
        if force {
            cmd = cmd.arg(["--force"]);
        }
        cmd = cmd.arg([branch]);

        let mut command = Command::new("git");
        command.args(cmd.into_args());
        command.env("GIT_TERMINAL_PROMPT", "0");
        for (key, value) in env_vars {
            command.env(key, value);
        }
        command.current_dir(PathBuf::from(&self.repo_id.0).as_path());

        let output = command
            .output()
            .map_err(|e| crate::GitError::OperationFailed {
                message: e.to_string(),
            })?;

        if !output.status.success() {
            return Err(crate::GitError::OperationFailed {
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
        Ok(())
    }

    pub fn set_current_branch_upstream(
        &self,
        remote_name: &str,
        remote_branch: &str,
    ) -> GitResult<()> {
        git(
            PathBuf::from(&self.repo_id.0).as_path(),
            [
                "branch",
                &format!("--set-upstream-to={}/{}", remote_name, remote_branch),
            ],
        )
    }

    pub fn set_upstream(
        &self,
        remote_name: &str,
        remote_branch: &str,
        branch_name: &str,
    ) -> GitResult<()> {
        git(
            PathBuf::from(&self.repo_id.0).as_path(),
            [
                "branch",
                &format!("--set-upstream-to={}/{}", remote_name, remote_branch),
                branch_name,
            ],
        )
    }

    pub fn unset_upstream(&self, branch_name: &str) -> GitResult<()> {
        git(
            PathBuf::from(&self.repo_id.0).as_path(),
            ["branch", "--unset-upstream", branch_name],
        )
    }

    pub fn get_push_difference_count(&self) -> (String, String) {
        self.get_commit_differences("HEAD@{u}", "HEAD")
    }

    pub fn get_pull_difference_count(&self) -> (String, String) {
        self.get_commit_differences("HEAD", "HEAD@{u}")
    }

    fn get_commit_differences(&self, from: &str, to: &str) -> (String, String) {
        let pushable = self.count_differences(to, from);
        let pullable = self.count_differences(from, to);
        (
            pushable.unwrap_or_else(|_| "?".to_string()),
            pullable.unwrap_or_else(|_| "?".to_string()),
        )
    }

    fn count_differences(&self, from: &str, to: &str) -> GitResult<String> {
        let output = git_builder_output(
            PathBuf::from(&self.repo_id.0).as_path(),
            GitCommandBuilder::new("rev-list").arg([&format!("{}..{}", from, to), "--count"]),
        )?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn is_head_detached(&self) -> bool {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.repo_id.0)
            .arg("symbolic-ref")
            .arg("-q")
            .arg("HEAD")
            .output();

        match output {
            Ok(o) => !o.status.success(),
            Err(_) => false,
        }
    }

    pub fn rename(&self, old_name: &str, new_name: &str) -> GitResult<()> {
        git(
            PathBuf::from(&self.repo_id.0).as_path(),
            ["branch", "--move", old_name, new_name],
        )
    }

    pub fn merge(&self, branch_name: &str, variant: MergeVariant) -> GitResult<()> {
        let extra_args: Vec<&str> = match variant {
            MergeVariant::Regular => vec![],
            MergeVariant::FastForward => vec!["--ff"],
            MergeVariant::NoFastForward => vec!["--no-ff"],
            MergeVariant::Squash => vec!["--squash", "--ff"],
        };

        let mut cmd = GitCommandBuilder::new("merge").arg(["--no-edit"]);
        for arg in extra_args {
            cmd = cmd.arg([arg]);
        }
        cmd = cmd.arg([branch_name]);

        git_builder(PathBuf::from(&self.repo_id.0).as_path(), cmd)
    }

    pub fn can_do_fast_forward_merge(&self, ref_name: &str) -> bool {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.repo_id.0)
            .arg("merge-base")
            .arg("--is-ancestor")
            .arg("HEAD")
            .arg(ref_name)
            .output();

        output.map(|o| o.status.success()).unwrap_or(false)
    }

    pub fn previous_ref(&self) -> GitResult<String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.repo_id.0)
            .arg("rev-parse")
            .arg("--symbolic-full-name")
            .arg("@{-1}")
            .output()
            .map_err(|e| crate::GitError::OperationFailed {
                message: e.to_string(),
            })?;

        if !output.status.success() {
            return Err(crate::GitError::OperationFailed {
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn update_branch_refs(&self, input: &str) -> GitResult<()> {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(&self.repo_id.0)
            .arg("update-ref")
            .arg("--stdin")
            .current_dir(PathBuf::from(&self.repo_id.0).as_path())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| crate::GitError::OperationFailed {
            message: e.to_string(),
        })?;

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin
                .write_all(input.as_bytes())
                .map_err(|e| crate::GitError::OperationFailed {
                    message: e.to_string(),
                })?;
        }

        let output = child.wait().map_err(|e| crate::GitError::OperationFailed {
            message: e.to_string(),
        })?;

        if !output.status.success() {
            return Err(crate::GitError::OperationFailed {
                message: "update-ref failed".to_string(),
            });
        }

        Ok(())
    }
}
