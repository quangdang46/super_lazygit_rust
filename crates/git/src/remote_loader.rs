use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::Path;

use super_lazygit_core::state::{RemoteBranchItem, RemoteItem};

use crate::{git_stdout, GitError};

fn git_stdout_lines(repo_path: &Path, args: &[&str]) -> Result<Vec<String>, GitError> {
    git_stdout(repo_path, args).map(|output| output.lines().map(|l| l.trim().to_string()).collect())
}

/// Get all remotes with their branches, sorted with "origin" first.
pub fn get_remotes(repo_path: &Path) -> Result<Vec<RemoteItem>, GitError> {
    let remote_branches = get_remote_branches(repo_path).unwrap_or_default();
    let mut remotes = read_remote_urls(repo_path);

    // Add branch counts
    for remote in &mut remotes {
        remote.branch_count = remote_branches
            .iter()
            .filter(|branch| branch.remote_name == remote.name)
            .count();
    }

    // Sort: origin first, then alphabetically
    remotes.sort_by(compare_remote_items);

    Ok(remotes)
}

fn read_remote_urls(repo_path: &Path) -> Vec<RemoteItem> {
    let mut remotes: BTreeMap<String, RemoteItem> = BTreeMap::new();

    // Read remote URLs from config
    if let Ok(output) = git_stdout_lines(
        repo_path,
        &["config", "--local", "--get-regexp", r"^remote\.[^.]+\.url$"],
    ) {
        for line in output {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }
            let key = parts[0];
            let url = parts[1];
            // key is "remote.<name>.url"; strip prefix and suffix to get the name
            if let Some(remote_name) = key
                .strip_prefix("remote.")
                .and_then(|s| s.strip_suffix(".url"))
            {
                let entry = remotes
                    .entry(remote_name.to_string())
                    .or_insert_with(|| RemoteItem {
                        name: remote_name.to_string(),
                        fetch_url: String::new(),
                        push_url: String::new(),
                        branch_count: 0,
                    });
                entry.fetch_url = url.to_string();
                entry.push_url = url.to_string();
            }
        }
    }

    // Read fetch/push URLs from `git remote -v`
    if let Ok(output) = git_stdout_lines(repo_path, &["remote", "-v"]) {
        for line in output {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                continue;
            }
            let name = parts[0];
            let url = parts[1];
            let direction = parts[2].trim_matches(|ch| ch == '(' || ch == ')');

            let remote = remotes
                .entry(name.to_string())
                .or_insert_with(|| RemoteItem {
                    name: name.to_string(),
                    fetch_url: String::new(),
                    push_url: String::new(),
                    branch_count: 0,
                });

            match direction {
                "fetch" => remote.fetch_url = url.to_string(),
                "push" => remote.push_url = url.to_string(),
                _ => {}
            }
        }
    }

    remotes.into_values().collect()
}

fn compare_remote_items(left: &RemoteItem, right: &RemoteItem) -> Ordering {
    match (left.name == "origin", right.name == "origin") {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => {
            let lower_cmp = left.name.to_lowercase().cmp(&right.name.to_lowercase());
            if lower_cmp == Ordering::Equal {
                left.name.cmp(&right.name)
            } else {
                lower_cmp
            }
        }
    }
}

/// Get remote branches grouped by remote name.
pub fn get_remote_branches(repo_path: &Path) -> Result<Vec<RemoteBranchItem>, GitError> {
    git_stdout(
        repo_path,
        ["for-each-ref", "--format=%(refname:short)", "refs/remotes"],
    )
    .map(|output| {
        output
            .lines()
            .filter_map(|line| {
                let name = line.trim();
                if name.is_empty() || name.ends_with("/HEAD") {
                    return None;
                }
                let (remote_name, branch_name) = name.split_once('/')?;
                if branch_name.is_empty() {
                    return None;
                }
                Some(RemoteBranchItem {
                    name: branch_name.to_string(),
                    remote_name: remote_name.to_string(),
                    branch_name: branch_name.to_string(),
                })
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_remote_items() {
        let origin = RemoteItem {
            name: "origin".to_string(),
            fetch_url: "https://github.com/example/repo".to_string(),
            push_url: "https://github.com/example/repo".to_string(),
            branch_count: 5,
        };

        let upstream = RemoteItem {
            name: "upstream".to_string(),
            fetch_url: "https://github.com/other/repo".to_string(),
            push_url: "https://github.com/other/repo".to_string(),
            branch_count: 3,
        };

        let beta = RemoteItem {
            name: "beta".to_string(),
            fetch_url: "https://github.com/beta/repo".to_string(),
            push_url: "https://github.com/beta/repo".to_string(),
            branch_count: 2,
        };

        // origin should come first
        assert_eq!(compare_remote_items(&origin, &upstream), Ordering::Less);
        assert_eq!(compare_remote_items(&upstream, &origin), Ordering::Greater);

        // alphabetical for non-origin
        assert_eq!(compare_remote_items(&beta, &upstream), Ordering::Less);
        assert_eq!(compare_remote_items(&upstream, &beta), Ordering::Greater);
    }

    #[test]
    fn test_remote_branch_full_name() {
        let branch = RemoteBranchItem {
            name: "main".to_string(),
            remote_name: "origin".to_string(),
            branch_name: "main".to_string(),
        };
        assert_eq!(branch.full_name(), "origin/main");
    }
}
