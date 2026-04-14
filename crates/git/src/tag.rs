use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;

use crate::GitCommandBuilder;
use crate::GitResult;

pub struct TagCommands {
    repo_path: PathBuf,
}

impl TagCommands {
    pub fn new(repo_path: PathBuf) -> Self {
        Self { repo_path }
    }

    pub fn create_lightweight_obj(
        &self,
        tag_name: &str,
        ref_: &str,
        force: bool,
    ) -> GitResult<Vec<OsString>> {
        let mut builder = GitCommandBuilder::new("tag")
            .arg_if(force, ["--force"])
            .arg(["--"])
            .arg([tag_name]);

        if !ref_.is_empty() {
            builder = builder.arg([ref_]);
        }

        Ok(builder.to_argv())
    }

    pub fn create_annotated_obj(
        &self,
        tag_name: &str,
        ref_: &str,
        msg: &str,
        force: bool,
    ) -> GitResult<Vec<OsString>> {
        let mut builder = GitCommandBuilder::new("tag")
            .arg([tag_name])
            .arg_if(force, ["--force"]);

        if !ref_.is_empty() {
            builder = builder.arg([ref_]);
        }

        builder = builder.arg(["-m", msg]);
        Ok(builder.to_argv())
    }

    pub fn has_tag(&self, tag_name: &str) -> bool {
        let output = self.run_git_command([
            "show-ref",
            "--tags",
            "--quiet",
            "--verify",
            "--",
            &format!("refs/tags/{tag_name}"),
        ]);

        output.map(|o| o.status.success()).unwrap_or(false)
    }

    pub fn local_delete(&self, tag_name: &str) -> GitResult<()> {
        let output = self.run_git_command(["tag", "-d", tag_name])?;

        if !output.status.success() {
            return Err(crate::GitError::OperationFailed {
                message: format!(
                    "git tag -d failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }

        Ok(())
    }

    pub fn push(&self, remote_name: &str, tag_name: &str) -> GitResult<()> {
        let output = self.run_git_command(["push", remote_name, "tag", tag_name])?;

        if !output.status.success() {
            return Err(crate::GitError::OperationFailed {
                message: format!(
                    "git push failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }

        Ok(())
    }

    pub fn show_annotation_info(&self, tag_name: &str) -> GitResult<String> {
        let output = self.run_git_command([
            "for-each-ref",
            "--format=Tagger:     %(taggername) %(taggeremail)%0aTaggerDate: %(taggerdate)%0a%0a%(contents)",
            &format!("refs/tags/{tag_name}"),
        ])?;

        if !output.status.success() {
            return Err(crate::GitError::OperationFailed {
                message: format!(
                    "git for-each-ref failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn is_tag_annotated(&self, tag_name: &str) -> GitResult<bool> {
        let output = self.run_git_command(["cat-file", "-t", &format!("refs/tags/{tag_name}")])?;

        if !output.status.success() {
            return Err(crate::GitError::OperationFailed {
                message: format!(
                    "git cat-file failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }

        let output_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(output_str == "tag")
    }

    fn run_git_command<I, S>(&self, args: I) -> GitResult<std::process::Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| crate::GitError::OperationFailed {
                message: format!("failed to execute git: {}", e),
            })?;

        Ok(output)
    }
}
