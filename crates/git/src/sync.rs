use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::GitCommandBuilder;
use crate::{
    git_builder, git_builder_with_env, git_output, git_stdout, git_stdout_allow_failure, GitError,
    GitResult,
};

const DEFAULT_MAIN_BRANCH_NAMES: [&str; 2] = ["master", "main"];

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyncPushOptions {
    pub force: bool,
    pub force_with_lease: bool,
    pub current_branch: String,
    pub upstream_remote: String,
    pub upstream_branch: String,
    pub set_upstream: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyncPullOptions {
    pub remote_name: String,
    pub branch_name: String,
    pub fast_forward_only: bool,
    pub worktree_git_dir: String,
    pub worktree_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AutoForwardBranchCandidate {
    name: String,
    full_ref: String,
    upstream_full_ref: String,
    commit_hash: String,
    is_head: bool,
}

pub fn build_push_command(opts: &SyncPushOptions) -> GitResult<GitCommandBuilder> {
    if !opts.upstream_branch.is_empty() && opts.upstream_remote.is_empty() {
        return Err(GitError::OperationFailed {
            message: "must specify origin when pushing to an explicit upstream branch".to_string(),
        });
    }

    let mut builder = GitCommandBuilder::new("push")
        .arg_if(opts.force, ["--force"])
        .arg_if(opts.force_with_lease, ["--force-with-lease"])
        .arg_if(opts.set_upstream, ["--set-upstream"]);

    if !opts.upstream_remote.is_empty() {
        builder = builder.arg([OsString::from(opts.upstream_remote.clone())]);
    }
    if !opts.upstream_branch.is_empty() {
        builder = builder.arg([OsString::from(format!(
            "refs/heads/{}:{}",
            opts.current_branch, opts.upstream_branch
        ))]);
    }

    Ok(builder)
}

fn fetch_command_builder(fetch_all: bool) -> GitCommandBuilder {
    GitCommandBuilder::new("fetch")
        .arg_if(fetch_all, ["--all"])
        .arg(["--no-write-fetch-head"])
}

pub fn build_pull_command(opts: &SyncPullOptions) -> GitCommandBuilder {
    let mut builder = GitCommandBuilder::new("pull")
        .arg(["--no-edit"])
        .arg_if(opts.fast_forward_only, ["--ff-only"]);

    if !opts.remote_name.is_empty() {
        builder = builder.arg([OsString::from(opts.remote_name.clone())]);
    }
    if !opts.branch_name.is_empty() {
        builder = builder.arg([OsString::from(format!("refs/heads/{}", opts.branch_name))]);
    }

    builder
        .worktree_path_if(!opts.worktree_path.is_empty(), opts.worktree_path.clone())
        .git_dir_if(
            !opts.worktree_git_dir.is_empty(),
            opts.worktree_git_dir.clone(),
        )
}

pub fn run_fetch(repo_path: &Path) -> GitResult<()> {
    if let Some(remote) = default_remote(repo_path)? {
        run_fetch_remote(repo_path, remote.as_str())
    } else {
        git_builder(repo_path, fetch_command_builder(true))?;
        auto_forward_default_branches(repo_path)
    }
}

pub fn run_fetch_remote(repo_path: &Path, remote_name: &str) -> GitResult<()> {
    git_builder(
        repo_path,
        fetch_command_builder(false).arg([OsString::from(remote_name)]),
    )?;
    auto_forward_default_branches(repo_path)
}

pub fn run_pull(repo_path: &Path) -> GitResult<()> {
    if has_upstream(repo_path)? {
        git_builder_with_env(
            repo_path,
            build_pull_command(&SyncPullOptions {
                fast_forward_only: true,
                ..SyncPullOptions::default()
            }),
            &[("GIT_SEQUENCE_EDITOR", OsStr::new(":"))],
        )
    } else {
        Err(GitError::OperationFailed {
            message: "pull requires an upstream tracking branch".to_string(),
        })
    }
}

pub fn run_push(repo_path: &Path) -> GitResult<()> {
    let builder = if has_upstream(repo_path)? {
        build_push_command(&SyncPushOptions::default())?
    } else {
        let branch = current_branch_name(repo_path)?;
        let remote = default_remote(repo_path)?.unwrap_or_else(|| "origin".to_string());
        build_push_command(&SyncPushOptions {
            current_branch: branch.clone(),
            upstream_remote: remote,
            upstream_branch: branch,
            set_upstream: true,
            ..SyncPushOptions::default()
        })?
    };

    git_builder(repo_path, builder)
}

fn default_remote(repo_path: &Path) -> GitResult<Option<String>> {
    let remote = git_stdout_allow_failure(repo_path, ["remote"])?;
    Ok(remote
        .lines()
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned))
}

fn has_upstream(repo_path: &Path) -> GitResult<bool> {
    Ok(!git_stdout_allow_failure(
        repo_path,
        [
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
    )?
    .is_empty())
}

fn current_branch_name(repo_path: &Path) -> GitResult<String> {
    let branch = git_stdout(repo_path, ["branch", "--show-current"])?;
    if branch.is_empty() {
        return Err(GitError::OperationFailed {
            message: "push requires an attached branch HEAD".to_string(),
        });
    }
    Ok(branch)
}

fn auto_forward_default_branches(repo_path: &Path) -> GitResult<()> {
    let update_commands = collect_auto_forward_branch_updates(repo_path)?;
    update_branch_refs(repo_path, &update_commands)
}

fn collect_auto_forward_branch_updates(repo_path: &Path) -> GitResult<String> {
    let checked_out_branch_refs = read_checked_out_branch_refs(repo_path)?;
    let mut update_commands = String::new();

    for branch in read_auto_forward_branch_candidates(repo_path)? {
        if branch.is_head
            || branch.upstream_full_ref.is_empty()
            || !DEFAULT_MAIN_BRANCH_NAMES.contains(&branch.name.as_str())
            || checked_out_branch_refs.contains(branch.full_ref.as_str())
            || !ref_exists(repo_path, branch.upstream_full_ref.as_str())?
        {
            continue;
        }

        let (ahead, behind) = branch_divergence_counts(
            repo_path,
            branch.full_ref.as_str(),
            branch.upstream_full_ref.as_str(),
        )?;
        if behind > 0 && ahead == 0 {
            update_commands.push_str(
                format!(
                    "update {} {} {}\n",
                    branch.full_ref, branch.upstream_full_ref, branch.commit_hash
                )
                .as_str(),
            );
        }
    }

    Ok(update_commands)
}

fn read_auto_forward_branch_candidates(
    repo_path: &Path,
) -> GitResult<Vec<AutoForwardBranchCandidate>> {
    git_stdout(
        repo_path,
        [
            "for-each-ref",
            "--format=%(HEAD)%00%(refname:short)%00%(refname)%00%(upstream)%00%(objectname)",
            "refs/heads",
        ],
    )
    .map(|output| {
        output
            .lines()
            .filter_map(parse_auto_forward_branch_candidate)
            .collect()
    })
}

fn parse_auto_forward_branch_candidate(line: &str) -> Option<AutoForwardBranchCandidate> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut parts = trimmed.split('\0');
    let head = parts.next().unwrap_or_default().trim();
    let name = normalize_local_branch_name(parts.next().unwrap_or_default().trim());
    let full_ref = parts.next().unwrap_or_default().trim().to_string();
    let upstream_full_ref = parts.next().unwrap_or_default().trim().to_string();
    let commit_hash = parts.next().unwrap_or_default().trim().to_string();

    if name.is_empty() || full_ref.is_empty() || commit_hash.is_empty() {
        return None;
    }

    Some(AutoForwardBranchCandidate {
        name: name.to_string(),
        full_ref,
        upstream_full_ref,
        commit_hash,
        is_head: head == "*",
    })
}

fn read_checked_out_branch_refs(repo_path: &Path) -> GitResult<std::collections::HashSet<String>> {
    Ok(
        git_stdout_allow_failure(repo_path, ["worktree", "list", "--porcelain"])?
            .lines()
            .filter_map(|line| line.strip_prefix("branch "))
            .map(str::trim)
            .filter(|branch_ref| !branch_ref.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

fn branch_divergence_counts(
    repo_path: &Path,
    local_ref: &str,
    upstream_ref: &str,
) -> GitResult<(usize, usize)> {
    let refspec = format!("{local_ref}...{upstream_ref}");
    let counts = git_stdout(
        repo_path,
        ["rev-list", "--left-right", "--count", refspec.as_str()],
    )?;
    let mut parts = counts.split_whitespace();
    let ahead = parse_divergence_count(parts.next())?;
    let behind = parse_divergence_count(parts.next())?;
    Ok((ahead, behind))
}

fn parse_divergence_count(raw: Option<&str>) -> GitResult<usize> {
    raw.unwrap_or_default()
        .parse::<usize>()
        .map_err(|error| GitError::OperationFailed {
            message: format!("failed to parse branch divergence count: {}", error),
        })
}

fn ref_exists(repo_path: &Path, reference: &str) -> GitResult<bool> {
    Ok(
        git_output(repo_path, ["rev-parse", "--verify", "--quiet", reference])?
            .status
            .success(),
    )
}

fn update_branch_refs(repo_path: &Path, update_commands: &str) -> GitResult<()> {
    if update_commands.trim().is_empty() {
        return Ok(());
    }
    git_with_stdin(
        repo_path,
        ["update-ref", "--stdin"],
        update_commands.as_bytes(),
    )
}

fn git_with_stdin<I, S>(repo_path: &Path, args: I, stdin: &[u8]) -> GitResult<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut child = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| GitError::OperationFailed {
            message: format!("failed to spawn git process: {}", e),
        })?;

    let mut stdin_handle = child.stdin.take().unwrap();
    stdin_handle
        .write_all(stdin)
        .map_err(|e| GitError::OperationFailed {
            message: format!("failed to write to git stdin: {}", e),
        })?;
    drop(stdin_handle);

    let output = child
        .wait_with_output()
        .map_err(|e| GitError::OperationFailed {
            message: format!("failed to wait for git process: {}", e),
        })?;

    if !output.status.success() {
        return Err(GitError::OperationFailed {
            message: format!(
                "git stdin command failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }
    Ok(())
}

fn normalize_local_branch_name(name: &str) -> &str {
    name.trim_start_matches("refs/heads/")
}
