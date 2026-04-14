use std::ffi::OsString;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::{git, git_builder, git_builder_output, git_stdout, GitCommandBuilder, GitResult};

pub fn drop_newest() -> GitResult<()> {
    git_builder(
        &Path::new("."),
        GitCommandBuilder::new("stash").arg(["drop"]),
    )
}

pub fn drop_stash(index: usize) -> GitResult<()> {
    git_builder(
        &Path::new("."),
        GitCommandBuilder::new("stash").arg(["drop", &format!("refs/stash@{{{index}}}")]),
    )
}

pub fn pop(index: usize) -> GitResult<()> {
    git_builder(
        &Path::new("."),
        GitCommandBuilder::new("stash").arg(["pop", &format!("refs/stash@{{{index}}}")]),
    )
}

pub fn apply(index: usize) -> GitResult<()> {
    git_builder(
        &Path::new("."),
        GitCommandBuilder::new("stash").arg(["apply", &format!("refs/stash@{{{index}}}")]),
    )
}

pub fn push(message: &str) -> GitResult<()> {
    git_builder(
        &Path::new("."),
        GitCommandBuilder::new("stash").arg(["push", "-m", message]),
    )
}

pub fn store(hash: &str, message: &str) -> GitResult<()> {
    let trimmed_message = message.trim();
    git_builder(
        &Path::new("."),
        GitCommandBuilder::new("stash")
            .arg(["store"])
            .arg_if(!trimmed_message.is_empty(), ["-m", trimmed_message])
            .arg([hash]),
    )
}

pub fn hash(index: usize) -> GitResult<String> {
    let output = git_stdout(
        &Path::new("."),
        ["rev-parse", &format!("refs/stash@{{{index}}}")],
    )?;
    Ok(output
        .trim_end_matches(|c| c == '\r' || c == '\n')
        .to_string())
}

pub fn show_stash_entry_cmd_obj(
    ext_diff_cmd: &str,
    use_ext_diff_git_config: bool,
    color_arg: &str,
    diff_context_size: usize,
    ignore_whitespace: bool,
    rename_similarity_threshold: u8,
    stash_ref: &str,
    worktree_path: &Path,
) -> String {
    let mut builder = GitCommandBuilder::new("stash")
        .arg(["show", "-p", "--stat", "-u"])
        .config_if(
            !ext_diff_cmd.is_empty(),
            format!("diff.external={}", ext_diff_cmd),
        )
        .arg_if_else(
            !ext_diff_cmd.is_empty() || use_ext_diff_git_config,
            "--ext-diff",
            "--no-ext-diff",
        )
        .arg([OsString::from(format!("--color={}", color_arg))])
        .arg([OsString::from(format!("--unified={}", diff_context_size))])
        .arg_if(ignore_whitespace, ["--ignore-all-space"])
        .arg([OsString::from(format!(
            "--find-renames={}%",
            rename_similarity_threshold
        ))])
        .arg([stash_ref])
        .dir(worktree_path);

    for arg in builder.to_argv().into_iter().skip(1) {
        print!("{} ", arg.to_string_lossy());
    }
    println!();
    String::new()
}

pub fn stash_and_keep_index(message: &str) -> GitResult<()> {
    git_builder(
        &Path::new("."),
        GitCommandBuilder::new("stash").arg(["push", "--keep-index", "-m", message]),
    )
}

pub fn stash_unstaged_changes(message: &str) -> GitResult<()> {
    git(
        &Path::new("."),
        [
            "commit",
            "--no-verify",
            "-m",
            "[lazygit] stashing unstaged changes",
        ],
    )?;
    push(message)?;
    git(&Path::new("."), ["reset", "--soft", "HEAD^"])
}

pub fn save_staged_changes(
    message: &str,
    version_is_at_least: impl Fn(u32, u32, u32) -> bool,
) -> GitResult<()> {
    if version_is_at_least(2, 35, 0) {
        return git_builder(
            &Path::new("."),
            GitCommandBuilder::new("stash").arg(["push", "--staged", "-m", message]),
        );
    }

    git_builder(
        &Path::new("."),
        GitCommandBuilder::new("stash").arg(["--keep-index"]),
    )?;
    push(message)?;
    git_builder(
        &Path::new("."),
        GitCommandBuilder::new("stash").arg(["apply", "refs/stash@{1}"]),
    )?;

    apply_stash_patch()?;

    git_builder(
        &Path::new("."),
        GitCommandBuilder::new("stash").arg(["drop", "refs/stash@{1}"]),
    )?;

    Ok(())
}

fn apply_stash_patch() -> GitResult<()> {
    let mut child = Command::new("git")
        .args(["apply", "-R"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| crate::GitError::OperationFailed {
            message: format!("failed to spawn git apply -R: {}", e),
        })?;

    let output = git_builder_output(
        &Path::new("."),
        GitCommandBuilder::new("stash").arg(["show", "-p"]),
    )?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(&output.stdout)
            .map_err(|e| crate::GitError::OperationFailed {
                message: format!("failed to write patch to stdin: {}", e),
            })?;
    }
    std::mem::drop(child.stdin.take());

    let result = child
        .wait_with_output()
        .map_err(|e| crate::GitError::OperationFailed {
            message: format!("failed to wait for git apply -R: {}", e),
        })?;

    if !result.status.success() {
        return Err(crate::GitError::OperationFailed {
            message: format!(
                "git apply -R failed: {}",
                String::from_utf8_lossy(&result.stderr)
            ),
        });
    }

    Ok(())
}

pub fn stash_include_untracked_changes(message: &str) -> GitResult<()> {
    git_builder(
        &Path::new("."),
        GitCommandBuilder::new("stash").arg(["push", "--include-untracked", "-m", message]),
    )
}

pub fn rename_stash(index: usize, message: &str) -> GitResult<()> {
    let h = hash(index)?;
    drop_stash(index)?;
    store(&h, message)
}
