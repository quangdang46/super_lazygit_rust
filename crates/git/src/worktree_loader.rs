use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super_lazygit_core::state::WorktreeItem;

use crate::{git_stdout, git_stdout_allow_failure, GitError};

fn worktree_path_missing(path: &Path) -> bool {
    match fs::metadata(path) {
        Ok(_) => false,
        Err(error) => error.kind() == std::io::ErrorKind::NotFound,
    }
}

fn worktree_git_dir(worktree_path: &Path) -> Option<PathBuf> {
    git_stdout_allow_failure(
        worktree_path,
        ["rev-parse", "--path-format=absolute", "--absolute-git-dir"],
    )
    .ok()
    .filter(|value| !value.is_empty())
    .and_then(|value| {
        let p = PathBuf::from(value.trim());
        // Try to canonicalize if it exists
        if fs::metadata(&p).is_ok() {
            std::fs::canonicalize(&p).ok()
        } else {
            Some(p)
        }
    })
}

fn rebased_branch(git_dir: &Path) -> Option<String> {
    for dir in ["rebase-merge", "rebase-apply"] {
        if let Ok(content) = fs::read_to_string(git_dir.join(dir).join("head-name")) {
            let value = content.trim();
            if !value.is_empty() {
                return Some(short_head_name(value));
            }
        }
    }
    None
}

fn bisected_branch(git_dir: &Path) -> Option<String> {
    fs::read_to_string(git_dir.join("BISECT_START"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn short_head_name(value: &str) -> String {
    value.trim().trim_start_matches("refs/heads/").to_string()
}

#[derive(Debug, Clone)]
struct IndexedPath {
    path: String,
    index: usize,
}

#[derive(Debug, Clone)]
struct IndexedName {
    name: String,
    index: usize,
}

fn unique_worktree_names(paths: &[String]) -> Vec<String> {
    let indexed_paths = paths
        .iter()
        .enumerate()
        .map(|(index, path)| IndexedPath {
            path: path.clone(),
            index,
        })
        .collect::<Vec<_>>();
    let indexed_names = unique_worktree_names_at_depth(indexed_paths, 0);
    let mut names = vec![String::new(); paths.len()];
    for indexed_name in indexed_names {
        names[indexed_name.index] = indexed_name.name;
    }
    names
}

fn unique_worktree_names_at_depth(paths: Vec<IndexedPath>, depth: usize) -> Vec<IndexedName> {
    if paths.is_empty() {
        return Vec::new();
    }
    if paths.len() == 1 {
        let path = &paths[0];
        return vec![IndexedName {
            index: path.index,
            name: slice_at_depth(&path.path, depth),
        }];
    }

    let mut groups: BTreeMap<String, Vec<IndexedPath>> = BTreeMap::new();
    for path in paths {
        let key = value_at_depth(&path.path, depth);
        groups.entry(key).or_default().push(path);
    }

    let mut names = Vec::new();
    for group in groups.into_values() {
        if group.len() == 1 {
            let path = &group[0];
            names.push(IndexedName {
                index: path.index,
                name: slice_at_depth(&path.path, depth),
            });
        } else {
            names.extend(unique_worktree_names_at_depth(group, depth + 1));
        }
    }
    names
}

fn value_at_depth(path: &str, depth: usize) -> String {
    let segments = normalized_path_segments(path);
    if depth >= segments.len() {
        String::new()
    } else {
        segments[segments.len() - 1 - depth].clone()
    }
}

fn slice_at_depth(path: &str, depth: usize) -> String {
    let segments = normalized_path_segments(path);
    if depth >= segments.len() {
        String::new()
    } else {
        segments[segments.len() - 1 - depth..].join("/")
    }
}

fn normalized_path_segments(path: &str) -> Vec<String> {
    path.replace('\\', "/")
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// Get all worktrees.
pub fn get_worktrees(repo_path: &Path) -> Result<Vec<WorktreeItem>, GitError> {
    let output = git_stdout(repo_path, ["worktree", "list", "--porcelain"])?;

    let repo_paths =
        crate::RepoPaths::resolve(repo_path).map_err(|e| GitError::OperationFailed {
            message: format!("failed to resolve repo paths: {e}"),
        })?;

    let mut items = Vec::new();
    let mut current: Option<WorktreeItem> = None;

    for line in output.lines() {
        if line.is_empty() {
            push_worktree_item(&mut items, &mut current);
            continue;
        }

        if line == "bare" {
            current = None;
            continue;
        }

        if let Some(path) = line.strip_prefix("worktree ") {
            push_worktree_item(&mut items, &mut current);

            let raw_path = PathBuf::from(path);
            let is_path_missing = worktree_path_missing(&raw_path);
            let path = if is_path_missing {
                raw_path.clone()
            } else {
                fs::canonicalize(&raw_path).unwrap_or(raw_path)
            };

            current = Some(WorktreeItem {
                is_main: path == repo_paths.repo_path(),
                is_current: path == repo_paths.worktree_path(),
                path: path.clone(),
                is_path_missing,
                ..WorktreeItem::default()
            });
            continue;
        }

        let Some(item) = current.as_mut() else {
            continue;
        };

        if let Some(head) = line.strip_prefix("HEAD ") {
            item.head = head.to_string();
            continue;
        }

        if let Some(branch) = line.strip_prefix("branch ") {
            item.branch = Some(short_head_name(branch));
        }
    }
    push_worktree_item(&mut items, &mut current);

    // Populate git_dir for each worktree
    for item in &mut items {
        if !item.is_path_missing {
            item.git_dir = worktree_git_dir(&item.path);
        }
    }

    // Assign unique names based on paths
    let names = unique_worktree_names(
        &items
            .iter()
            .map(|item| item.path.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
    );
    for (item, name) in items.iter_mut().zip(names) {
        item.name = name;
    }

    // Move current worktree to the top
    if let Some(index) = items.iter().position(|item| item.is_current) {
        let current_item = items.remove(index);
        items.insert(0, current_item);
    }

    // Check for rebased/bisected branch names
    for item in &mut items {
        if item.branch.is_some() {
            continue;
        }
        let Some(git_dir) = item.git_dir.as_deref() else {
            continue;
        };
        if let Some(branch) = rebased_branch(git_dir) {
            item.branch = Some(branch);
            continue;
        }
        if let Some(branch) = bisected_branch(git_dir) {
            item.branch = Some(branch);
        }
    }

    Ok(items)
}

fn push_worktree_item(items: &mut Vec<WorktreeItem>, current: &mut Option<WorktreeItem>) {
    if let Some(item) = current.take() {
        items.push(item);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unique_worktree_names() {
        // Basic cases
        assert_eq!(unique_worktree_names(&[]), Vec::<String>::new());

        let paths = vec!["/my/path/feature/one".to_string()];
        assert_eq!(unique_worktree_names(&paths), vec!["one"]);

        // Multiple levels
        let paths = vec![
            "/my/path/feature/one".to_string(),
            "/my/path/feature/two".to_string(),
        ];
        assert_eq!(unique_worktree_names(&paths), vec!["one", "two"]);

        // Disambiguating with parent directories
        let paths = vec![
            "/my/path/feature/one".to_string(),
            "/my/path/feature-one".to_string(),
        ];
        let names = unique_worktree_names(&paths);
        assert!(names.contains(&"one".to_string()));
        assert!(names.contains(&"feature-one".to_string()));
    }

    #[test]
    fn test_short_head_name() {
        assert_eq!(short_head_name("refs/heads/main"), "main");
        assert_eq!(short_head_name("main"), "main");
        assert_eq!(
            short_head_name("  refs/heads/feature/test  "),
            "feature/test"
        );
    }

    #[test]
    fn test_normalized_path_segments() {
        assert_eq!(normalized_path_segments("/a/b/c"), vec!["a", "b", "c"]);
        assert_eq!(normalized_path_segments("a/b/c"), vec!["a", "b", "c"]);
        assert_eq!(normalized_path_segments("/a//b///c/"), vec!["a", "b", "c"]);
        assert_eq!(normalized_path_segments(r"\a\b\c"), vec!["a", "b", "c"]);
    }

    #[test]
    fn test_value_at_depth() {
        assert_eq!(value_at_depth("/a/b/c/d", 0), "d");
        assert_eq!(value_at_depth("/a/b/c/d", 1), "c");
        assert_eq!(value_at_depth("/a/b/c/d", 2), "b");
        assert_eq!(value_at_depth("/a/b/c/d", 3), "a");
        assert_eq!(value_at_depth("/a/b/c/d", 4), "");
    }

    #[test]
    fn test_slice_at_depth() {
        assert_eq!(slice_at_depth("/a/b/c/d", 0), "d");
        assert_eq!(slice_at_depth("/a/b/c/d", 1), "c/d");
        assert_eq!(slice_at_depth("/a/b/c/d", 2), "b/c/d");
        assert_eq!(slice_at_depth("/a/b/c/d", 3), "a/b/c/d");
        assert_eq!(slice_at_depth("/a/b/c/d", 4), "");
    }
}
