use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Output};

use super_lazygit_core::{GitCommand, ResetMode};

use crate::GitCommandBuilder;
use crate::GitError;
use crate::GitResult;

pub fn stage_file(repo_path: &Path, path: &Path) -> GitResult<()> {
    git_path(repo_path, ["add"], path)
}

pub fn discard_file(repo_path: &Path, path: &Path) -> GitResult<()> {
    discard_path(repo_path, path)
}

pub fn unstage_file(repo_path: &Path, path: &Path) -> GitResult<()> {
    unstage_path(repo_path, path)
}

pub fn unstage_selection(repo_path: &Path) -> GitResult<()> {
    git(
        repo_path,
        ["restore", "--staged", "--source=HEAD", "--", "."],
    )
}

pub fn reset_to_commit(repo_path: &Path, mode: ResetMode, target: &str) -> GitResult<()> {
    git(repo_path, ["reset", reset_mode_flag(mode), target])
}

pub fn nuke_working_tree(repo_path: &Path) -> GitResult<()> {
    git(repo_path, ["reset", "--hard", "HEAD"])?;
    git(repo_path, ["clean", "-fd"])
}

pub fn discard_path(repo_path: &Path, path: &Path) -> GitResult<()> {
    if path_exists_in_head(repo_path, path)? {
        git_path(
            repo_path,
            ["restore", "--source=HEAD", "--staged", "--worktree"],
            path,
        )
    } else if path_exists_in_index(repo_path, path)? {
        git_path(repo_path, ["rm", "-f"], path)
    } else {
        git_path(repo_path, ["clean", "-f"], path)
    }
}

fn unstage_path(repo_path: &Path, path: &Path) -> GitResult<()> {
    let restore = git_path_output(repo_path, ["restore", "--staged"], path)?;
    if restore.status.success() {
        return Ok(());
    }

    let rm_cached = git_path_output(repo_path, ["rm", "--cached"], path)?;
    if rm_cached.status.success() {
        return Ok(());
    }

    Err(GitError::OperationFailed {
        message: format!(
            "git restore --staged failed:\n{}\n\ngit rm --cached failed:\n{}",
            command_failure_message(restore),
            command_failure_message(rm_cached)
        ),
    })
}

fn path_exists_in_head(repo_path: &Path, path: &Path) -> GitResult<bool> {
    let spec = format!("HEAD:{}", path.to_string_lossy());
    let output = Command::new("git")
        .arg("cat-file")
        .arg("-e")
        .arg(spec)
        .current_dir(repo_path)
        .output()
        .map_err(io_error)?;
    Ok(output.status.success())
}

fn path_exists_in_index(repo_path: &Path, path: &Path) -> GitResult<bool> {
    let output = git_path_output(repo_path, ["ls-files", "--error-unmatch", "--cached"], path)?;
    Ok(output.status.success())
}

fn reset_mode_flag(mode: ResetMode) -> &'static str {
    match mode {
        ResetMode::Soft => "--soft",
        ResetMode::Mixed => "--mixed",
        ResetMode::Hard => "--hard",
    }
}

fn io_error(error: std::io::Error) -> GitError {
    GitError::OperationFailed {
        message: error.to_string(),
    }
}

fn git<I, S>(repo_path: &Path, args: I) -> GitResult<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = git_output(repo_path, args)?;
    if !output.status.success() {
        return Err(command_failure(output));
    }
    Ok(())
}

fn git_output<I, S>(repo_path: &Path, args: I) -> GitResult<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = Command::new("git");
    cmd.current_dir(repo_path).args(args);
    cmd.output().map_err(|e| GitError::OperationFailed {
        message: format!("failed to execute git: {}", e),
    })
}

fn command_failure(output: Output) -> GitError {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    GitError::OperationFailed {
        message: format!(
            "git exited with status {}\nstdout:\n{}\nstderr:\n{}",
            output.status, stdout, stderr
        ),
    }
}

fn command_failure_message(output: Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.is_empty() {
        format!("git command failed: {}", stdout)
    } else {
        format!("git command failed: {} {}", stdout, stderr)
    }
}

fn git_builder_output(repo_path: &Path, builder: GitCommandBuilder) -> GitResult<Output> {
    let output = git_output(repo_path, builder.to_argv())?;
    if !output.status.success() {
        return Err(command_failure(output));
    }
    Ok(output)
}

fn build_git_path_command<I, S>(args: I, path: &Path) -> GitResult<GitCommandBuilder>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut args = args.into_iter();
    let command = args
        .next()
        .ok_or_else(|| GitError::OperationFailed {
            message: "git path command requires at least one argument".to_string(),
        })?
        .as_ref()
        .to_os_string();

    Ok(GitCommandBuilder::new(command)
        .arg(args.map(|arg| arg.as_ref().to_os_string()))
        .arg(["--"])
        .arg([path.as_os_str().to_os_string()]))
}

fn git_path<I, S>(repo_path: &Path, args: I, path: &Path) -> GitResult<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = git_builder_output(repo_path, build_git_path_command(args, path)?)?;
    if !output.status.success() {
        return Err(command_failure(output));
    }
    Ok(())
}

fn git_path_output<I, S>(repo_path: &Path, args: I, path: &Path) -> GitResult<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    git_builder_output(repo_path, build_git_path_command(args, path)?)
}

pub fn handle_working_tree_command(
    command: &GitCommand,
    repo_path: &Path,
) -> Result<String, GitError> {
    match command {
        GitCommand::StageSelection => {
            git_path(repo_path, ["add", "-A"], Path::new("."))?;
            Ok("Staged current selection".to_string())
        }
        GitCommand::UnstageSelection => {
            unstage_selection(repo_path)?;
            Ok("Unstaged current selection".to_string())
        }
        GitCommand::StageFile { path } => {
            stage_file(repo_path, path)?;
            Ok(format!("Staged {}", path.display()))
        }
        GitCommand::DiscardFile { path } => {
            discard_file(repo_path, path)?;
            Ok(format!("Discarded changes for {}", path.display()))
        }
        GitCommand::UnstageFile { path } => {
            unstage_file(repo_path, path)?;
            Ok(format!("Unstaged {}", path.display()))
        }
        GitCommand::ResetToCommit { mode, target } => {
            reset_to_commit(repo_path, *mode, target)?;
            let short = git_stdout(repo_path, ["rev-parse", "--short", target.as_str()])
                .unwrap_or_else(|_| target.clone());
            let subject = git_stdout(repo_path, ["show", "-s", "--format=%s", target])
                .unwrap_or_else(|_| target.clone());
            Ok(format!("{} reset to {} {}", mode.title(), short, subject))
        }
        GitCommand::RestoreSnapshot { target } => {
            reset_to_commit(repo_path, ResetMode::Hard, target)?;
            Ok(format!("Restored HEAD to {target}"))
        }
        GitCommand::NukeWorkingTree => {
            nuke_working_tree(repo_path)?;
            Ok("Discarded all local changes".to_string())
        }
        _ => Err(GitError::OperationFailed {
            message: format!("not a working tree command: {:?}", command),
        }),
    }
}

fn git_stdout<I, S>(repo_path: &Path, args: I) -> GitResult<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = git_output(repo_path, args)?;
    if !output.status.success() {
        return Err(command_failure(output));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
