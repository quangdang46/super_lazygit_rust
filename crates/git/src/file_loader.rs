use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super_lazygit_core::state::{FileStatus, FileStatusKind};

use crate::GitCommandBuilder;
use crate::GitResult;

const RENAME_SIMILARITY_THRESHOLD: u8 = 50;

pub struct FileLoaderConfig {
    pub get_show_untracked_files: Box<dyn Fn() -> String + Send + Sync>,
}

impl FileLoaderConfig {
    #[must_use]
    pub fn get_show_untracked_files(&self) -> String {
        (self.get_show_untracked_files)()
    }
}

pub struct FileLoader {
    repo_path: PathBuf,
    config: FileLoaderConfig,
}

impl FileLoader {
    pub fn new(repo_path: PathBuf, config: FileLoaderConfig) -> Self {
        Self { repo_path, config }
    }

    pub fn get_status_files(&self, opts: GetStatusFileOptions) -> Vec<FileStatus> {
        // check if config wants us ignoring untracked files
        let mut untracked_files_setting = self.config.get_show_untracked_files();

        if opts.force_show_untracked || untracked_files_setting.is_empty() {
            untracked_files_setting = "all".to_string();
        }
        let untracked_files_arg = format!("--untracked-files={}", untracked_files_setting);

        let statuses = self.git_status(GitStatusOptions {
            no_renames: opts.no_renames,
            untracked_files_arg,
        });

        let file_diffs = self.get_file_diffs().unwrap_or_default();

        let mut files: Vec<FileStatus> = statuses
            .into_iter()
            .filter(|status| !status.status_string.starts_with("warning"))
            .map(|status| {
                let mut file = FileStatus {
                    path: PathBuf::from(&status.path),
                    previous_path: status.previous_path.map(PathBuf::from),
                    kind: FileStatusKind::Modified,
                    staged_kind: None,
                    unstaged_kind: None,
                    short_status: status.change.clone(),
                    inline_merge_conflicts: None,
                    display_string: status.status_string.clone(),
                    lines_added: 0,
                    lines_deleted: 0,
                    is_worktree: false,
                };

                if let Some(diff) = file_diffs.get(&status.path) {
                    file.lines_added = diff.lines_added;
                    file.lines_deleted = diff.lines_deleted;
                }

                let derived = FileStatus::derived_status_fields(&status.change);
                file.staged_kind = if derived.has_staged_changes {
                    Some(kind_from_status_char(
                        status.change.chars().next().unwrap_or(' '),
                    ))
                } else {
                    None
                };
                file.unstaged_kind = if derived.has_unstaged_changes {
                    Some(kind_from_status_char(
                        status.change.chars().nth(1).unwrap_or(' '),
                    ))
                } else {
                    None
                };
                file.inline_merge_conflicts = Some(derived.has_inline_merge_conflicts);

                file
            })
            .collect();

        // Mark worktree entries
        mark_worktree_entries(&self.repo_path, &mut files);

        files
    }

    fn get_file_diffs(&self) -> GitResult<std::collections::BTreeMap<String, FileDiff>> {
        let output = git_output(
            &self.repo_path,
            GitCommandBuilder::new("diff")
                .arg(["--numstat", "-z", "HEAD"])
                .to_argv(),
        )?;

        if !output.status.success() {
            return Ok(std::collections::BTreeMap::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut file_diffs = std::collections::BTreeMap::new();

        for line in stdout.split('\0') {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() != 3 {
                continue;
            }

            let lines_added: u32 = parts[0].parse().unwrap_or(0);
            let lines_deleted: u32 = parts[1].parse().unwrap_or(0);
            let file_name = parts[2].to_string();

            file_diffs.insert(
                file_name,
                FileDiff {
                    lines_added,
                    lines_deleted,
                },
            );
        }

        Ok(file_diffs)
    }

    fn git_status(&self, opts: GitStatusOptions) -> Vec<FileStatusLine> {
        let mut builder = GitCommandBuilder::new("status")
            .arg([&opts.untracked_files_arg])
            .arg(["--porcelain"])
            .arg(["-z"]);

        if opts.no_renames {
            builder = builder.arg(["--no-renames"]);
        } else {
            builder = builder.arg([format!("--find-renames={}%", RENAME_SIMILARITY_THRESHOLD)]);
        }

        let output = match git_output(&self.repo_path, builder.to_argv()) {
            Ok(output) if output.status.success() => output,
            _ => return Vec::new(),
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut response = Vec::new();

        for (i, original) in stdout.split('\0').enumerate() {
            if original.len() < 3 {
                continue;
            }

            let change = original[..2].to_string();
            let path = original[3..].to_string();
            let mut previous_path = None;
            let mut status_string = original.to_string();

            if change.starts_with('R') || change.starts_with('C') {
                // For renames/copies, the next entry is the original path
                if let Some(next_part) = stdout.split('\0').nth(i + 1) {
                    previous_path = Some(next_part.to_string());
                    status_string = format!("{} {} -> {}", change, next_part, path);
                }
            }

            response.push(FileStatusLine {
                status_string,
                change,
                path,
                previous_path,
            });
        }

        response
    }
}

fn kind_from_status_char(ch: char) -> FileStatusKind {
    match ch {
        'M' => FileStatusKind::Modified,
        'A' => FileStatusKind::Added,
        'D' => FileStatusKind::Deleted,
        'R' => FileStatusKind::Renamed,
        '?' => FileStatusKind::Untracked,
        'U' => FileStatusKind::Conflicted,
        _ => FileStatusKind::Modified,
    }
}

fn mark_worktree_entries(repo_path: &Path, files: &mut [FileStatus]) {
    let worktree_paths = read_worktree_paths(repo_path);

    for item in files.iter_mut() {
        let absolute_path = normalized_worktree_path(&repo_path.join(&item.path));
        if worktree_paths.contains(&absolute_path) {
            item.is_worktree = true;
            item.path = trimmed_status_path(&item.path);
        }
    }
}

fn read_worktree_paths(repo_path: &Path) -> HashSet<PathBuf> {
    let output = match git_output(repo_path, ["worktree", "list", "--porcelain"]) {
        Ok(output) if output.status.success() => output,
        _ => return HashSet::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut paths = HashSet::new();

    for line in stdout.lines() {
        if let Some(worktree_path) = line.strip_prefix("worktree ") {
            let path = PathBuf::from(worktree_path);
            if path.exists() {
                paths.insert(path);
            }
        }
    }

    paths
}

fn normalized_worktree_path(path: &Path) -> PathBuf {
    trimmed_status_path(path)
}

fn trimmed_status_path(path: &Path) -> PathBuf {
    let value = path.to_string_lossy();
    PathBuf::from(value.trim_end_matches('/'))
}

fn git_output<I>(repo_path: &Path, args: I) -> GitResult<std::process::Output>
where
    I: IntoIterator,
    I::Item: AsRef<std::ffi::OsStr>,
{
    use std::process::Command;

    let mut cmd = Command::new("git");
    cmd.current_dir(repo_path).args(args);

    cmd.output().map_err(|e| crate::GitError::OperationFailed {
        message: format!("failed to execute git: {}", e),
    })
}

#[derive(Default)]
pub struct GetStatusFileOptions {
    pub no_renames: bool,
    /// If true, we'll show untracked files even if the user has set the config to hide them.
    /// This is useful for users with bare repos for dotfiles who default to hiding untracked files,
    /// but want to occasionally see them to `git add` a new file.
    pub force_show_untracked: bool,
}

struct GitStatusOptions {
    no_renames: bool,
    untracked_files_arg: String,
}

struct FileStatusLine {
    status_string: String,
    change: String,
    path: String,
    previous_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FileDiff {
    pub lines_added: u32,
    pub lines_deleted: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kind_from_status_char() {
        assert!(matches!(
            kind_from_status_char('M'),
            FileStatusKind::Modified
        ));
        assert!(matches!(kind_from_status_char('A'), FileStatusKind::Added));
        assert!(matches!(
            kind_from_status_char('D'),
            FileStatusKind::Deleted
        ));
        assert!(matches!(
            kind_from_status_char('R'),
            FileStatusKind::Renamed
        ));
        assert!(matches!(
            kind_from_status_char('?'),
            FileStatusKind::Untracked
        ));
        assert!(matches!(
            kind_from_status_char('U'),
            FileStatusKind::Conflicted
        ));
    }

    #[test]
    fn test_trimmed_status_path() {
        let path = PathBuf::from("foo/bar/");
        assert_eq!(trimmed_status_path(&path), PathBuf::from("foo/bar"));
    }

    #[test]
    fn test_get_status_file_options_default() {
        let opts = GetStatusFileOptions::default();
        assert!(!opts.no_renames);
        assert!(!opts.force_show_untracked);
    }
}
