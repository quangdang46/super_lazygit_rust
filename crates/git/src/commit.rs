use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::git_builder;
use crate::git_builder_output;
use crate::git_stdout;
use crate::{GitCommandBuilder, GitError, GitResult};

const ERR_INVALID_COMMIT_INDEX: &str = "invalid commit index";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Author {
    pub name: String,
    pub email: String,
}

pub struct CommitCommands {
    repo_path: PathBuf,
}

impl CommitCommands {
    #[must_use]
    pub fn new(repo_path: PathBuf) -> Self {
        Self { repo_path }
    }

    pub fn reset_author(&self) -> GitResult<()> {
        git_builder(
            &self.repo_path,
            GitCommandBuilder::new("commit").arg([
                "--allow-empty",
                "--allow-empty-message",
                "--only",
                "--no-edit",
                "--amend",
                "--reset-author",
            ]),
        )
    }

    pub fn set_author(&self, value: &str) -> GitResult<()> {
        git_builder(
            &self.repo_path,
            GitCommandBuilder::new("commit")
                .arg([
                    "--allow-empty",
                    "--allow-empty-message",
                    "--only",
                    "--no-edit",
                    "--amend",
                ])
                .arg(["--author=".to_owned() + value]),
        )
    }

    pub fn add_co_author(&self, hash: &str, author: &str) -> GitResult<()> {
        let message = get_commit_message(&self.repo_path, hash)?;
        let message = add_co_author_to_message(&message, author);

        git_builder(
            &self.repo_path,
            GitCommandBuilder::new("commit").arg([
                "--allow-empty",
                "--amend",
                "--only",
                "-m",
                &message,
            ]),
        )
    }
}

pub fn add_co_author_to_message(message: &str, author: &str) -> String {
    let (subject, body) = message.split_once('\n').unwrap_or((message, ""));
    let subject = subject.trim();
    let body = body.trim();

    if body.is_empty() {
        subject.to_string()
    } else {
        format!(
            "{}\n\n{}",
            subject,
            add_co_author_to_description(body, author)
        )
    }
}

pub fn add_co_author_to_description(description: &str, author: &str) -> String {
    let mut desc = description.to_string();
    if !desc.is_empty() {
        let lines: Vec<&str> = desc.split('\n').collect();
        if lines
            .last()
            .map_or(false, |line| line.starts_with("Co-authored-by:"))
        {
            desc.push('\n');
        } else {
            desc.push_str("\n\n");
        }
    }
    format!("{}Co-authored-by: {}", desc, author)
}

pub fn reset_to_commit(
    repo_path: &Path,
    hash: &str,
    strength: &str,
    env_vars: &[(&str, &str)],
) -> GitResult<()> {
    let mut cmd = Command::new("git");
    cmd.args(["reset", &format!("--{}", strength), hash])
        .current_dir(repo_path)
        .env("GIT_TERMINAL_PROMPT", "0");
    for (key, value) in env_vars {
        cmd.env(key, value);
    }

    let output = cmd.output().map_err(|e| GitError::OperationFailed {
        message: format!("failed to execute git reset: {}", e),
    })?;

    if !output.status.success() {
        return Err(command_failure(output));
    }
    Ok(())
}

pub fn get_commit_message(repo_path: &Path, commit_hash: &str) -> GitResult<String> {
    let output = git_stdout(
        repo_path,
        [
            "log",
            "--format=%B",
            "--max-count=1",
            commit_hash,
            "-c",
            "log.showsignature=false",
        ],
    )?;
    Ok(normalize_linefeeds(&output))
}

pub fn get_commit_subject(repo_path: &Path, commit_hash: &str) -> GitResult<String> {
    let subject = git_stdout(
        repo_path,
        [
            "log",
            "--format=%s",
            "--max-count=1",
            commit_hash,
            "-c",
            "log.showsignature=false",
        ],
    )?;
    Ok(subject.trim().to_string())
}

pub fn get_commit_diff(repo_path: &Path, commit_hash: &str) -> GitResult<String> {
    git_stdout(repo_path, ["show", "--no-color", commit_hash])
}

pub fn get_commit_author(repo_path: &Path, commit_hash: &str) -> GitResult<Author> {
    let output = git_builder_output(
        repo_path,
        GitCommandBuilder::new("show")
            .arg(["--no-patch", "--pretty=format:%an%x00%ae"])
            .arg([commit_hash]),
    )?;

    if !output.status.success() {
        return Err(command_failure(output));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    let mut parts = trimmed.split('\0');
    let name = parts.next().unwrap_or_default().to_string();
    let email = parts.next().unwrap_or_default().to_string();

    if name.is_empty() || email.is_empty() {
        return Err(GitError::OperationFailed {
            message: "unexpected git output".to_string(),
        });
    }

    Ok(Author { name, email })
}

pub fn get_commit_message_first_line(repo_path: &Path, hash: &str) -> GitResult<String> {
    get_commit_messages_first_line(repo_path, &[hash])
}

pub fn get_commit_messages_first_line(repo_path: &Path, hashes: &[&str]) -> GitResult<String> {
    let mut args = vec!["show", "--no-patch", "--pretty=format:%s"];
    for h in hashes {
        args.push(h);
    }
    git_stdout(repo_path, &args)
}

pub fn get_commits_oneline(repo_path: &Path, hashes: &[&str]) -> GitResult<String> {
    let mut args = vec!["show", "--no-patch", "--oneline"];
    for h in hashes {
        args.push(h);
    }
    git_stdout(repo_path, &args)
}

pub fn amend_head(repo_path: &Path) -> GitResult<()> {
    git_builder(
        repo_path,
        GitCommandBuilder::new("commit").arg([
            "--amend",
            "--no-edit",
            "--allow-empty",
            "--allow-empty-message",
        ]),
    )
}

pub fn amend_head_cmd_obj() -> GitCommandBuilder {
    GitCommandBuilder::new("commit").arg([
        "--amend",
        "--no-edit",
        "--allow-empty",
        "--allow-empty-message",
    ])
}

pub fn show_cmd_obj(
    repo_path: &Path,
    hash: &str,
    filter_paths: &[&str],
    context_size: usize,
    use_external_diff: bool,
    external_diff_cmd: &str,
    color_arg: &str,
    ignore_whitespace: bool,
    rename_threshold: u8,
) -> GitCommandBuilder {
    let mut builder = GitCommandBuilder::new("show")
        .config("diff.noprefix=false")
        .arg_if(
            !external_diff_cmd.is_empty(),
            [format!("diff.external={}", external_diff_cmd)],
        )
        .arg_if_else(
            use_external_diff || !external_diff_cmd.is_empty(),
            "--ext-diff",
            "--no-ext-diff",
        )
        .arg(["--submodule"])
        .arg(["--color=".to_owned() + color_arg])
        .arg(["--unified=".to_string() + &context_size.to_string()])
        .arg(["--stat"])
        .arg(["--decorate"])
        .arg(["-p"])
        .arg([hash])
        .arg_if(ignore_whitespace, ["--ignore-all-space"])
        .arg(["--find-renames=".to_string() + &format!("{}%", rename_threshold)]);

    if !filter_paths.is_empty() {
        builder = builder.arg(["--"]).arg(
            filter_paths
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<String>>(),
        );
    }

    builder.dir(repo_path)
}

pub fn show_file_content_cmd_obj(hash: &str, file_path: &str) -> GitCommandBuilder {
    GitCommandBuilder::new("show").arg([format!("{}:{}", hash, file_path)])
}

pub fn revert(repo_path: &Path, hashes: &[&str], is_merge: bool) -> GitResult<()> {
    let mut builder = GitCommandBuilder::new("revert");
    if is_merge {
        builder = builder.arg(["-m", "1"]);
    }
    builder = builder.arg(hashes.iter().map(|s| s.to_string()).collect::<Vec<_>>());

    git_builder(repo_path, builder)
}

pub fn create_fixup_commit(repo_path: &Path, hash: &str) -> GitResult<()> {
    git_builder(
        repo_path,
        GitCommandBuilder::new("commit").arg(["--fixup=".to_owned() + hash]),
    )
}

pub fn create_amend_commit(
    repo_path: &Path,
    original_subject: &str,
    new_subject: &str,
    new_description: &str,
    include_file_changes: bool,
) -> GitResult<()> {
    let description = if new_description.is_empty() {
        new_subject.to_string()
    } else {
        format!("{}\n\n{}", new_subject, new_description)
    };

    let mut builder = GitCommandBuilder::new("commit")
        .arg(["-m", &format!("amend! {}", original_subject)])
        .arg(["-m", &description]);

    if !include_file_changes {
        builder = builder.arg(["--only", "--allow-empty"]);
    }

    git_builder(repo_path, builder)
}

pub fn get_commit_message_from_history(repo_path: &Path, value: usize) -> GitResult<String> {
    let hash = git_stdout(
        repo_path,
        ["log", "-1", &format!("--skip={}", value), "--pretty=%H"],
    )?;
    let formatted_hash = hash.trim();
    if formatted_hash.is_empty() {
        return Err(GitError::OperationFailed {
            message: ERR_INVALID_COMMIT_INDEX.to_string(),
        });
    }
    get_commit_message(repo_path, formatted_hash)
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

fn normalize_linefeeds(s: &str) -> String {
    s.replace("\r\n", "\n").trim().to_string()
}
