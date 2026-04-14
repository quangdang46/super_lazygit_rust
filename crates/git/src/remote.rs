use std::path::Path;

use crate::{git, git_stdout, GitResult};

pub struct RemoteCommands;

impl RemoteCommands {
    pub fn new() -> Self {
        Self
    }

    pub fn add_remote(&self, repo_path: &Path, name: &str, url: &str) -> GitResult<()> {
        git(repo_path, ["remote", "add", name, url])
    }

    pub fn remove_remote(&self, repo_path: &Path, name: &str) -> GitResult<()> {
        git(repo_path, ["remote", "remove", name])
    }

    pub fn rename_remote(
        &self,
        repo_path: &Path,
        old_remote_name: &str,
        new_remote_name: &str,
    ) -> GitResult<()> {
        git(
            repo_path,
            ["remote", "rename", old_remote_name, new_remote_name],
        )
    }

    pub fn update_remote_url(
        &self,
        repo_path: &Path,
        remote_name: &str,
        updated_url: &str,
    ) -> GitResult<()> {
        git(repo_path, ["remote", "set-url", remote_name, updated_url])
    }

    pub fn delete_remote_branch(
        &self,
        repo_path: &Path,
        remote_name: &str,
        branch_names: &[String],
    ) -> GitResult<()> {
        let refs: Vec<String> = branch_names
            .iter()
            .map(|b| format!("refs/heads/{}", b))
            .collect();

        let mut args = vec!["push", remote_name, "--delete"];
        for r in &refs {
            args.push(r);
        }

        git(repo_path, args)
    }

    pub fn delete_remote_tag(
        &self,
        repo_path: &Path,
        remote_name: &str,
        tag_name: &str,
    ) -> GitResult<()> {
        git(
            repo_path,
            [
                "push",
                remote_name,
                "--delete",
                &format!("refs/tags/{}", tag_name),
            ],
        )
    }

    pub fn check_remote_branch_exists(&self, repo_path: &Path, branch_name: &str) -> bool {
        let result = git_stdout(
            repo_path,
            [
                "show-ref",
                "--verify",
                "--",
                &format!("refs/remotes/origin/{}", branch_name),
            ],
        );
        result.is_ok()
    }

    pub fn get_remote_url(&self, repo_path: &Path, remote_name: &str) -> GitResult<String> {
        let url = git_stdout(repo_path, ["ls-remote", "--get-url", remote_name])?;
        Ok(url.trim().to_string())
    }
}

impl Default for RemoteCommands {
    fn default() -> Self {
        Self::new()
    }
}
