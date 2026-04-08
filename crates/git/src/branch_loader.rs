use std::collections::HashSet;
use std::process::Command;

use crate::GitError;
use crate::GitResult;
use super_lazygit_core::{BranchItem, GitRef, ReflogItem, RepoId};

/// BranchInfo holds information about the current branch state.
#[derive(Debug, Clone)]
pub struct BranchInfo {
    pub ref_name: String,
    pub display_name: String,
    pub detached_head: bool,
}

/// BranchLoader loads and manages branch information for a repository.
pub struct BranchLoader {
    repo_id: RepoId,
}

impl BranchLoader {
    /// Create a new BranchLoader instance.
    #[must_use]
    pub fn new(repo_id: RepoId) -> Self {
        Self { repo_id }
    }

    /// Load the list of branches for the current repo.
    ///
    /// # Arguments
    ///
    /// * `reflog_commits` - Reflog commits to use for recency matching
    /// * `old_branches` - Previous branch list to preserve BehindBaseBranch values
    /// * `local_branch_sort_order` - Sort order for branches ("recency", "date", or "alphabetical")
    pub fn load(
        &self,
        reflog_commits: &[ReflogItem],
        old_branches: &[BranchItem],
        local_branch_sort_order: &str,
    ) -> GitResult<Vec<BranchItem>> {
        let mut branches = self.obtain_branches()?;

        if local_branch_sort_order.eq_ignore_ascii_case("recency")
            || local_branch_sort_order.eq_ignore_ascii_case("date")
        {
            let reflog_branches = self.obtain_reflog_branches(reflog_commits);
            // Loop through reflog branches. If there is a match, merge them, then remove it from the branches and keep it in the reflog branches
            let mut branches_with_recency: Vec<BranchItem> = Vec::new();
            let mut to_remove: Vec<usize> = Vec::new();

            for reflog_branch in &reflog_branches {
                for (j, branch) in branches.iter().enumerate() {
                    if branch.is_head {
                        continue;
                    }
                    if branch.name.eq_ignore_ascii_case(&reflog_branch.name) {
                        let mut branch = branch.clone();
                        branch.recency = reflog_branch.recency.clone();
                        branches_with_recency.push(branch);
                        to_remove.push(j);
                        break;
                    }
                }
            }

            // Remove marked branches (in reverse order to maintain indices)
            for j in to_remove.into_iter().rev() {
                branches.remove(j);
            }

            // Sort remaining branches alphabetically for deterministic behaviour across git versions
            branches.sort_by(|a, b| a.name.cmp(&b.name));

            // Prepend branches with recency
            branches_with_recency.append(&mut branches);
            branches = branches_with_recency;
        }

        // Find the head branch and move it to the front
        let mut found_head = false;
        for (i, branch) in branches.iter().enumerate() {
            if branch.is_head {
                found_head = true;
                let mut branch = branch.clone();
                branch.recency = "  *".to_string();
                branches.remove(i);
                branches.insert(0, branch);
                break;
            }
        }

        if !found_head {
            let info = self.get_current_branch_info()?;
            branches.insert(
                0,
                BranchItem {
                    name: info.ref_name,
                    display_name: Some(info.display_name),
                    is_head: true,
                    detached_head: info.detached_head,
                    ..Default::default()
                },
            );
        }

        // If the branch already existed, take over its BehindBaseBranch value to reduce flicker
        for branch in branches.iter_mut() {
            if let Some(old_branch) = old_branches.iter().find(|b| b.name == branch.name) {
                branch.behind_base_branch = old_branch.behind_base_branch;
            }
        }

        Ok(branches)
    }

    /// Find the base branch for the given branch (i.e. the main branch that the given branch was forked off of).
    ///
    /// Note that this function may return an empty string even if the returned error is nil,
    /// e.g. when none of the configured main branches exist. This is not considered an error
    /// condition, so callers need to check both the returned error and whether the returned
    /// base branch is empty.
    pub fn get_base_branch(
        &self,
        branch: &BranchItem,
        main_branches: &[String],
    ) -> GitResult<String> {
        let merge_base = self.get_merge_base(&branch.full_ref_name(), main_branches);
        if merge_base.is_empty() {
            return Ok(String::new());
        }

        let output = Command::new("git")
            .arg("-C")
            .arg(&self.repo_id.0)
            .arg("for-each-ref")
            .arg("--contains")
            .arg(&merge_base)
            .arg("--format=%(refname)")
            .args(main_branches)
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let trimmed = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let split: Vec<&str> = trimmed.split('\n').collect();
                if split.is_empty() || split[0].is_empty() {
                    Ok(String::new())
                } else {
                    Ok(split[0].to_string())
                }
            }
            Ok(output) => Err(GitError::OperationFailed {
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            }),
            Err(e) => Err(GitError::OperationFailed {
                message: e.to_string(),
            }),
        }
    }

    /// Get merge base of a branch with the main branches.
    fn get_merge_base(&self, ref_name: &str, main_branches: &[String]) -> String {
        if main_branches.is_empty() {
            return String::new();
        }

        let output = Command::new("git")
            .arg("-C")
            .arg(&self.repo_id.0)
            .arg("merge-base")
            .arg(ref_name)
            .args(main_branches)
            .output();

        output
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default()
    }

    fn obtain_branches(&self) -> GitResult<Vec<BranchItem>> {
        let output = self.get_raw_branches()?;
        let trimmed = output.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        let lines: Vec<&str> = trimmed.lines().collect();
        let mut branches = Vec::new();

        for line in lines {
            if line.is_empty() {
                continue;
            }

            let split: Vec<&str> = line.split('\x00').collect();
            if split.len() != BRANCH_FIELDS.len() {
                // Ignore line if it isn't separated into the expected number of parts
                // This is probably a warning message
                continue;
            }

            branches.push(Self::obtain_branch(split));
        }

        Ok(branches)
    }

    fn get_raw_branches(&self) -> GitResult<String> {
        let format = BRANCH_FIELDS
            .iter()
            .map(|f| format!("%({})", f))
            .collect::<Vec<_>>()
            .join("\x00");

        let output = Command::new("git")
            .arg("-C")
            .arg(&self.repo_id.0)
            .arg("for-each-ref")
            .arg("--sort=-committerdate")
            .arg(format!("--format={}", format))
            .arg("refs/heads")
            .output()
            .map_err(|e| GitError::OperationFailed {
                message: e.to_string(),
            })?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(GitError::OperationFailed {
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            })
        }
    }

    /// Obtain branch information from parsed line output of getRawBranches()
    fn obtain_branch(split: Vec<&str>) -> BranchItem {
        let head_marker = split[0];
        let full_name = split[1];
        let upstream_name = split[2];
        let track = split[3];
        let push_track = split[4];
        let subject = split[5];
        let commit_hash = split[6];
        let _commit_date = split[7];

        let name = full_name
            .strip_prefix("heads/")
            .unwrap_or(full_name)
            .to_string();
        let (ahead_for_pull, behind_for_pull, gone) =
            Self::parse_upstream_info(upstream_name, track);
        let (ahead_for_push, behind_for_push, _) =
            Self::parse_upstream_info(upstream_name, push_track);

        BranchItem {
            name,
            display_name: None,
            is_head: head_marker == "*",
            detached_head: false,
            upstream: None,
            recency: String::new(),
            ahead_for_pull,
            behind_for_pull,
            ahead_for_push,
            behind_for_push,
            upstream_gone: gone,
            upstream_remote: None,
            upstream_branch: None,
            subject: subject.to_string(),
            commit_hash: commit_hash.to_string(),
            commit_timestamp: None,
            behind_base_branch: 0,
        }
    }

    /// Parse upstream tracking info from git output.
    fn parse_upstream_info(upstream_name: &str, track: &str) -> (String, String, bool) {
        if upstream_name.is_empty() {
            // If we're here then it means we do not have a local version of the remote.
            // The branch might still be tracking a remote though, we just don't know
            // how many commits ahead/behind it is
            return ("?".to_string(), "?".to_string(), false);
        }

        if track == "[gone]" {
            return ("?".to_string(), "?".to_string(), true);
        }

        let ahead = Self::parse_difference(track, r"ahead \(\d+\)");
        let behind = Self::parse_difference(track, r"behind \(\d+\)");

        (ahead, behind, false)
    }

    fn parse_difference(track: &str, regex_str: &str) -> String {
        // Simple regex matching
        let re = regex::Regex::new(regex_str).ok();
        if let Some(re) = re {
            if let Some(captures) = re.captures(track) {
                if let Some(m) = captures.get(1) {
                    return m.as_str().to_string();
                }
            }
        }
        "0".to_string()
    }

    /// Obtain branches from reflog commits.
    fn obtain_reflog_branches(&self, reflog_commits: &[ReflogItem]) -> Vec<BranchItem> {
        let mut found_branches: HashSet<String> = HashSet::new();
        let re = regex::Regex::new(r"checkout: moving from ([\S]+) to ([\S]+)").unwrap();
        let mut reflog_branches: Vec<BranchItem> = Vec::new();

        for commit in reflog_commits {
            if let Some(captures) = re.captures(&commit.summary) {
                if captures.len() != 3 {
                    continue;
                }

                // Get both branch names from the captures
                for i in 1..3 {
                    if let Some(branch_name) = captures.get(i) {
                        let name = branch_name.as_str().to_string();
                        if !found_branches.contains(&name) {
                            found_branches.insert(name.clone());
                            reflog_branches.push(BranchItem {
                                name,
                                display_name: None,
                                is_head: false,
                                detached_head: false,
                                upstream: None,
                                recency: Self::unix_to_time_ago(commit.unix_timestamp),
                                ahead_for_pull: String::new(),
                                behind_for_pull: String::new(),
                                ahead_for_push: String::new(),
                                behind_for_push: String::new(),
                                upstream_gone: false,
                                upstream_remote: None,
                                upstream_branch: None,
                                subject: String::new(),
                                commit_hash: String::new(),
                                commit_timestamp: None,
                                behind_base_branch: 0,
                            });
                        }
                    }
                }
            }
        }

        reflog_branches
    }

    /// Convert Unix timestamp to time ago string.
    fn unix_to_time_ago(unix_timestamp: i64) -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if unix_timestamp <= 0 {
            return String::new();
        }

        let diff = now.saturating_sub(unix_timestamp as u64);

        if diff < 60 {
            format!("{}s", diff)
        } else if diff < 3600 {
            format!("{}m", diff / 60)
        } else if diff < 86400 {
            format!("{}h", diff / 3600)
        } else if diff < 604800 {
            format!("{}d", diff / 86400)
        } else if diff < 2592000 {
            format!("{}w", diff / 604800)
        } else if diff < 31536000 {
            format!("{}mo", diff / 2592000)
        } else {
            format!("{}y", diff / 31536000)
        }
    }

    /// Get current branch information.
    pub fn get_current_branch_info(&self) -> GitResult<BranchInfo> {
        // Try symbolic-ref first
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.repo_id.0)
            .arg("symbolic-ref")
            .arg("--short")
            .arg("HEAD")
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let branch_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !branch_name.is_empty() && branch_name != "HEAD" {
                    return Ok(BranchInfo {
                        ref_name: branch_name.clone(),
                        display_name: branch_name,
                        detached_head: false,
                    });
                }
            }
        }

        // Try branch --points-at=HEAD
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.repo_id.0)
            .arg("branch")
            .arg("--points-at=HEAD")
            .arg("--format=%(HEAD)%00%(objectname)%00%(refname)")
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                for line in String::from_utf8_lossy(&output.stdout).lines() {
                    let line = line.trim_end_matches('\r');
                    let split: Vec<&str> = line.split('\x00').collect();
                    if split.len() == 3 && split[0] == "*" {
                        let hash = split[1].to_string();
                        let display_name = split[2].to_string();
                        return Ok(BranchInfo {
                            ref_name: hash,
                            display_name,
                            detached_head: true,
                        });
                    }
                }
            }
        }

        Ok(BranchInfo {
            ref_name: "HEAD".to_string(),
            display_name: "HEAD".to_string(),
            detached_head: true,
        })
    }
}

/// Branch field names used in git for-each-ref format.
const BRANCH_FIELDS: &[&str] = &[
    "HEAD",
    "refname:short",
    "upstream:short",
    "upstream:track",
    "push:track",
    "subject",
    "objectname",
    "committerdate:unix",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_upstream_info_empty() {
        let (ahead, behind, gone) = BranchLoader::parse_upstream_info("", "");
        assert_eq!(ahead, "?");
        assert_eq!(behind, "?");
        assert!(!gone);
    }

    #[test]
    fn test_parse_upstream_info_gone() {
        let (ahead, behind, gone) = BranchLoader::parse_upstream_info("origin/main", "[gone]");
        assert_eq!(ahead, "?");
        assert_eq!(behind, "?");
        assert!(gone);
    }

    #[test]
    fn test_parse_upstream_info_ahead_behind() {
        let (ahead, behind, gone) =
            BranchLoader::parse_upstream_info("origin/main", "ahead 3, behind 2");
        assert_eq!(ahead, "3");
        assert_eq!(behind, "2");
        assert!(!gone);
    }

    #[test]
    fn test_parse_difference() {
        assert_eq!(
            BranchLoader::parse_difference("ahead 5", r"ahead \(\d+\)"),
            "5"
        );
        assert_eq!(
            BranchLoader::parse_difference("behind 10", r"behind \(\d+\)"),
            "10"
        );
        assert_eq!(
            BranchLoader::parse_difference("nothing", r"ahead \(\d+\)"),
            "0"
        );
    }

    #[test]
    fn test_unix_to_time_ago() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        // Seconds
        let result = BranchLoader::unix_to_time_ago(now - 30);
        assert!(result.ends_with('s'));

        // Minutes
        let result = BranchLoader::unix_to_time_ago(now - 120);
        assert!(result.ends_with('m'));

        // Hours
        let result = BranchLoader::unix_to_time_ago(now - 7200);
        assert!(result.ends_with('h'));

        // Days
        let result = BranchLoader::unix_to_time_ago(now - 172800);
        assert!(result.ends_with('d'));
    }

    #[test]
    fn test_branch_loader_creation() {
        let loader = BranchLoader::new(RepoId::new("/tmp"));
        assert!(loader.get_current_branch_info().is_ok());
    }
}
