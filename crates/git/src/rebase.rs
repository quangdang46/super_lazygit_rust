use std::ffi::OsStr;
use std::fs;
use std::io;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::{git_output_with_env, GitError, GitResult};
use super_lazygit_core::{RebaseKind, RebaseStartMode, RebaseState};

const GIT_OPTIONAL_LOCKS_ENV: &str = "GIT_OPTIONAL_LOCKS";
const GIT_OPTIONAL_LOCKS_DISABLED: &str = "0";

pub struct RebaseCommands;

impl RebaseCommands {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RebaseCommands {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
pub fn read_rebase_state(repo_path: &Path) -> Option<RebaseState> {
    let merge_dir = resolve_git_path(repo_path, "rebase-merge").filter(|path| path.exists());
    let apply_dir = resolve_git_path(repo_path, "rebase-apply").filter(|path| path.exists());
    let (dir, kind) = if let Some(dir) = merge_dir {
        let interactive = dir.join("interactive").exists();
        (
            dir,
            if interactive {
                RebaseKind::Interactive
            } else {
                RebaseKind::Apply
            },
        )
    } else if let Some(dir) = apply_dir {
        (dir, RebaseKind::Apply)
    } else {
        return None;
    };

    let step = read_usize_file(&dir.join("msgnum"))
        .or_else(|| read_usize_file(&dir.join("next")))
        .unwrap_or(0);
    let total = read_usize_file(&dir.join("end"))
        .or_else(|| read_usize_file(&dir.join("last")))
        .unwrap_or(step);
    let head_name = read_trimmed_file(&dir.join("head-name")).map(|s| normalize_head_name(&s));
    let onto = read_trimmed_file(&dir.join("onto"));
    let current_commit = git_stdout(repo_path, ["rev-parse", "--verify", "REBASE_HEAD"])
        .ok()
        .or_else(|| read_trimmed_file(&dir.join("stopped-sha")));
    let current_summary = current_commit
        .as_ref()
        .and_then(|c| git_stdout(repo_path, ["show", "-s", "--format=%s", c]).ok());

    Some(RebaseState {
        kind,
        step,
        total,
        head_name,
        onto,
        current_commit,
        current_summary,
        todo_preview: read_rebase_todo_preview(&dir),
    })
}

pub fn read_rebase_todo_preview(dir: &Path) -> Vec<String> {
    read_trimmed_file(&dir.join("git-rebase-todo"))
        .map(|contents| {
            contents
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .take(3)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub fn is_rebase_in_progress(repo_path: &Path) -> bool {
    git_path_exists(repo_path, "rebase-merge") || git_path_exists(repo_path, "rebase-apply")
}

pub fn rebased_branch(git_dir: &Path) -> Option<String> {
    ["rebase-merge", "rebase-apply"]
        .into_iter()
        .find_map(|dir| {
            read_trimmed_file(&git_dir.join(dir).join("head-name"))
                .map(|value| short_head_name(&value))
        })
}

pub fn run_start_commit_rebase(
    repo_path: &Path,
    commit: &str,
    mode: &RebaseStartMode,
) -> GitResult<()> {
    match mode {
        RebaseStartMode::Interactive | RebaseStartMode::Amend => {
            run_scripted_rebase(repo_path, commit, "edit", None, false)
        }
        RebaseStartMode::Fixup => {
            git(repo_path, ["commit", "--fixup", commit])?;
            run_scripted_rebase(repo_path, commit, "pick", None, true)
        }
        RebaseStartMode::FixupWithMessage => {
            run_scripted_rebase(repo_path, commit, "fixup -C", None, false)
        }
        RebaseStartMode::ApplyFixups => run_scripted_rebase(repo_path, commit, "pick", None, true),
        RebaseStartMode::Squash => run_scripted_rebase(repo_path, commit, "squash", None, false),
        RebaseStartMode::Drop => run_scripted_rebase(repo_path, commit, "drop", None, false),
        RebaseStartMode::MoveUp { adjacent_commit } => {
            run_reordered_rebase(repo_path, commit, adjacent_commit, true)
        }
        RebaseStartMode::MoveDown { adjacent_commit } => {
            run_reordered_rebase(repo_path, commit, adjacent_commit, false)
        }
        RebaseStartMode::Reword { message } => {
            run_scripted_rebase(repo_path, commit, "reword", Some(message.as_str()), false)
        }
    }
}

pub fn amend_commit_attributes(
    repo_path: &Path,
    commit: &str,
    reset_author: bool,
    co_author: Option<&str>,
) -> GitResult<()> {
    let resolved_commit = git_stdout(repo_path, ["rev-parse", commit])?;
    let head_commit = git_stdout(repo_path, ["rev-parse", "HEAD"])?;
    if resolved_commit == head_commit {
        amend_current_commit_attributes(repo_path, reset_author, co_author)?;
    } else {
        run_scripted_rebase(repo_path, &resolved_commit, "edit", None, false)?;
        amend_current_commit_attributes(repo_path, reset_author, co_author)?;
        git_with_env(
            repo_path,
            ["rebase", "--continue"],
            &[("GIT_EDITOR", OsStr::new(":"))],
        )?;
    }
    Ok(())
}

fn amend_current_commit_attributes(
    repo_path: &Path,
    reset_author: bool,
    co_author: Option<&str>,
) -> GitResult<()> {
    let mut args = vec![
        "commit".to_string(),
        "--amend".to_string(),
        "--allow-empty".to_string(),
        "--allow-empty-message".to_string(),
        "--only".to_string(),
        "--no-edit".to_string(),
    ];
    if reset_author {
        args.push("--reset-author".to_string());
    }
    if let Some(co_author) = co_author {
        args.push("--trailer".to_string());
        args.push(co_author.to_string());
    }
    git(repo_path, args)
}

fn run_scripted_rebase(
    repo_path: &Path,
    commit: &str,
    todo_verb: &str,
    reword_message: Option<&str>,
    autosquash: bool,
) -> GitResult<()> {
    let tempdir = tempfile::tempdir().map_err(io_error)?;
    let sequence_editor = tempdir.path().join("sequence-editor.sh");
    let sequence_script = if autosquash {
        "#!/bin/sh\nset -eu\n:\n".to_string()
    } else {
        format!(
            "#!/bin/sh\nset -eu\nfile=\"$1\"\ntmp=\"$1.tmp\"\nawk 'BEGIN{{done=0}} {{ if (!done && $1 == \"pick\" && index(\"{commit}\", $2) == 1) {{ sub(/^pick /, \"{todo_verb} \"); done=1 }} print }}' \"$file\" > \"$tmp\"\nmv \"$tmp\" \"$file\"\n"
        )
    };
    write_executable_script(&sequence_editor, &sequence_script)?;

    let editor_path = tempdir.path().join("git-editor.sh");
    let sequence_editor_command = git_script_command(&sequence_editor);
    let editor_command = if reword_message.is_some() {
        write_executable_script(
            &editor_path,
            "#!/bin/sh\nset -eu\nprintf '%s\\n' \"$SUPER_LAZYGIT_REWORD\" > \"$1\"\n",
        )?;
        Some(git_script_command(&editor_path))
    } else {
        None
    };
    let mut envs: Vec<(&str, &OsStr)> =
        vec![("GIT_SEQUENCE_EDITOR", sequence_editor_command.as_os_str())];

    if let Some(message) = reword_message {
        let editor_command = editor_command
            .as_ref()
            .expect("editor command should exist when rewording");
        envs.push(("GIT_EDITOR", editor_command.as_os_str()));
        envs.push(("SUPER_LAZYGIT_REWORD", OsStr::new(message)));
    } else {
        envs.push(("GIT_EDITOR", OsStr::new(":")));
    }

    let mut args = vec!["rebase".to_string(), "-i".to_string()];
    if autosquash {
        args.push("--autosquash".to_string());
    }
    if todo_verb == "squash" || todo_verb.starts_with("fixup") {
        let parent_commit = format!("{commit}^");
        let parent = git_stdout(repo_path, ["rev-parse", &parent_commit])?;
        args.extend(rebase_base_args(repo_path, &parent));
    } else {
        args.extend(rebase_base_args(repo_path, commit));
    }

    git_with_env(repo_path, args.iter().map(String::as_str), &envs)
}

fn run_reordered_rebase(
    repo_path: &Path,
    commit: &str,
    adjacent_commit: &str,
    move_up: bool,
) -> GitResult<()> {
    let (older, newer) = if move_up {
        (commit, adjacent_commit)
    } else {
        (adjacent_commit, commit)
    };
    let tempdir = tempfile::tempdir().map_err(io_error)?;
    let sequence_editor = tempdir.path().join("sequence-editor.sh");
    let sequence_script = format!(
        "#!/bin/sh\nset -eu\nfile=\"$1\"\ntmp=\"$1.tmp\"\nawk 'BEGIN{{swapped=0; older=\"{older}\"; newer=\"{newer}\"}} {{ if (!swapped && $1 == \"pick\" && index(older, $2) == 1) {{ older_line=$0; if ((getline newer_line) <= 0) {{ print older_line; next }} split(newer_line, newer_fields, \" \"); if (newer_fields[1] == \"pick\" && index(newer, newer_fields[2]) == 1) {{ print newer_line; print older_line; swapped=1; next }} print older_line; print newer_line; next }} print }} END {{ if (!swapped) exit 3 }}' \"$file\" > \"$tmp\"\nmv \"$tmp\" \"$file\"\n"
    );
    write_executable_script(&sequence_editor, &sequence_script)?;

    let sequence_editor_command = git_script_command(&sequence_editor);
    let envs: Vec<(&str, &OsStr)> = vec![
        ("GIT_SEQUENCE_EDITOR", sequence_editor_command.as_os_str()),
        ("GIT_EDITOR", OsStr::new(":")),
    ];
    let mut args = vec!["rebase".to_string(), "-i".to_string()];
    args.extend(rebase_base_args(repo_path, older));
    git_with_env(repo_path, args.iter().map(String::as_str), &envs)
}

fn write_executable_script(path: &Path, contents: &str) -> GitResult<()> {
    fs::write(path, contents).map_err(io_error)?;
    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(path).map_err(io_error)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).map_err(io_error)?;
    }
    Ok(())
}

fn git_script_command(path: &Path) -> std::ffi::OsString {
    #[cfg(windows)]
    {
        std::ffi::OsString::from(format!("sh {}", path.to_string_lossy().replace('\\', "/")))
    }

    #[cfg(not(windows))]
    {
        path.as_os_str().to_os_string()
    }
}

fn rebase_base_args(repo_path: &Path, commit: &str) -> Vec<String> {
    let parent_commit = format!("{commit}^");
    git_stdout(repo_path, ["rev-parse", &parent_commit])
        .map(|parent| vec![parent])
        .unwrap_or_else(|_| vec!["--root".to_string()])
}

fn read_trimmed_file(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn read_usize_file(path: &Path) -> Option<usize> {
    read_trimmed_file(path).and_then(|value| value.parse::<usize>().ok())
}

fn git_path_exists(repo_path: &Path, git_path: &str) -> bool {
    resolve_git_path(repo_path, git_path).is_some_and(|path| path.exists())
}

fn resolve_git_path(repo_path: &Path, git_path: &str) -> Option<PathBuf> {
    git_stdout(repo_path, ["rev-parse", "--git-path", git_path])
        .ok()
        .map(PathBuf::from)
}

fn git_stdout<I, S>(repo_path: &Path, args: I) -> GitResult<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = git_output_with_env(repo_path, args, &[])?;
    if !output.status.success() {
        return Err(GitError::OperationFailed {
            message: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn git_with_env<I, S>(repo_path: &Path, args: I, envs: &[(&str, &OsStr)]) -> GitResult<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = git_output_with_env(repo_path, args, envs)?;
    if !output.status.success() {
        return Err(GitError::OperationFailed {
            message: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(())
}

fn git<I, S>(repo_path: &Path, args: I) -> GitResult<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    git_output_with_env(repo_path, args, &[])?;
    Ok(())
}

fn io_error(err: io::Error) -> GitError {
    GitError::OperationFailed {
        message: err.to_string(),
    }
}

fn normalize_head_name(value: &str) -> String {
    value
        .strip_prefix("refs/heads/")
        .or_else(|| value.strip_prefix("refs/remotes/"))
        .or_else(|| value.strip_prefix("heads/"))
        .or_else(|| value.strip_prefix("remotes/"))
        .unwrap_or(value)
        .to_string()
}

fn short_head_name(value: &str) -> String {
    value
        .trim()
        .trim_start_matches("refs/heads/")
        .trim_start_matches("heads/")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_head_name() {
        assert_eq!(short_head_name("refs/heads/main"), "main");
        assert_eq!(short_head_name("main"), "main");
        assert_eq!(short_head_name("heads/feature/test"), "feature/test");
    }

    #[test]
    fn test_normalize_head_name() {
        assert_eq!(normalize_head_name("refs/heads/main"), "main");
        assert_eq!(
            normalize_head_name("refs/remotes/origin/main"),
            "origin/main"
        );
    }
}
