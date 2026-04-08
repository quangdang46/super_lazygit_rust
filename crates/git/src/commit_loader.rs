use std::collections::HashMap;
use std::ffi::OsString;
use std::path::Path;

use regex::Regex;

use super_lazygit_core::state::{CommitDivergence, CommitItem, CommitStatus, CommitTodoAction};

fn divergence_order(d: &CommitDivergence) -> i32 {
    match d {
        CommitDivergence::Left => 2,
        CommitDivergence::Right => 1,
        CommitDivergence::None => 0,
    }
}
use super_lazygit_core::WorkingTreeState;

use crate::{GitCommandBuilder, GitResult};

const PRETTY_FORMAT: &str = "--pretty=format:+%H%x00%at%x00%aN%x00%ae%x00%P%x00%m%x00%D%x00%s";

#[derive(Debug, Clone, Default)]
pub struct GetCommitsOptions {
    pub limit: bool,
    pub filter_path: Option<String>,
    pub filter_author: Option<String>,
    pub include_rebase_commits: bool,
    pub ref_name: String,
    pub all: bool,
    pub ref_to_show_divergence_from: Option<String>,
    pub main_branches: Vec<String>,
}

pub struct CommitLoader {
    repo_path: std::path::PathBuf,
    get_working_tree_state: Box<dyn Fn() -> WorkingTreeState + Send + Sync>,
}

impl CommitLoader {
    pub fn new(
        repo_path: impl Into<std::path::PathBuf>,
        get_working_tree_state: impl Fn() -> WorkingTreeState + Send + Sync + 'static,
    ) -> Self {
        Self {
            repo_path: repo_path.into(),
            get_working_tree_state: Box::new(get_working_tree_state),
        }
    }

    /// Get commits with various filtering options
    pub fn get_commits(&self, opts: GetCommitsOptions) -> GitResult<Vec<CommitItem>> {
        let mut commits = Vec::new();

        // Handle rebase commits if requested
        if opts.include_rebase_commits && opts.filter_path.is_none() {
            commits = self.merge_rebasing_commits(commits)?;
        }

        // Get log command and load commits
        let cmd = self.get_log_cmd(opts.clone());
        let mut all_commits = load_commits_from_cmd(&cmd, opts.filter_path.as_deref())?;

        // Get reachable hashes for determining merge status
        let unmerged_commit_hashes = if !opts.main_branches.is_empty() {
            self.get_reachable_hashes(&opts.ref_name, &opts.main_branches)
        } else {
            HashMap::new()
        };

        all_commits.extend(commits);

        if all_commits.is_empty() {
            return Ok(all_commits);
        }

        // Set commit statuses based on reachability
        set_commit_statuses(&unmerged_commit_hashes, &mut all_commits);

        // Handle divergence sorting if needed
        if opts.ref_to_show_divergence_from.is_some() {
            all_commits.sort_by(|a, b| {
                // Higher divergence value means "more incoming" so it comes first
                divergence_order(&b.divergence).cmp(&divergence_order(&a.divergence))
            });
        }

        Ok(all_commits)
    }

    fn merge_rebasing_commits(&self, commits: Vec<CommitItem>) -> GitResult<Vec<CommitItem>> {
        // Remove existing rebase commits - find the first non-TODO and take all from there
        let mut result: Vec<CommitItem> = Vec::with_capacity(commits.len());
        let mut found_non_todo = false;
        for commit in &commits {
            if !commit.is_todo() {
                found_non_todo = true;
            }
            if found_non_todo {
                result.push(commit.clone());
            }
        }

        let working_tree_state = (self.get_working_tree_state)();

        if working_tree_state.cherry_picking || working_tree_state.reverting {
            let sequencer_commits = self.get_hydrated_sequencer_commits(working_tree_state)?;
            result.splice(0..0, sequencer_commits);
        }

        if working_tree_state.rebasing {
            let rebasing_commits = self.get_hydrated_rebasing_commits(true)?;
            if !rebasing_commits.is_empty() {
                result.splice(0..0, rebasing_commits);
            }
        }

        Ok(result)
    }

    fn get_hydrated_rebasing_commits(
        &self,
        add_conflicting_commit: bool,
    ) -> GitResult<Vec<CommitItem>> {
        let todo_commits = self.get_rebasing_commits(add_conflicting_commit);
        self.get_hydrated_todo_commits(todo_commits, false)
    }

    fn get_hydrated_sequencer_commits(
        &self,
        working_tree_state: WorkingTreeState,
    ) -> GitResult<Vec<CommitItem>> {
        let mut commits = self.get_sequencer_commits();
        if !commits.is_empty() {
            // Last commit is the conflicting one
            if let Some(last) = commits.last_mut() {
                last.status = CommitStatus::Conflicted;
            }
        } else {
            // For single-commit cherry-picks/reverts, check CHERRY_PICK_HEAD or REVERT_HEAD
            if let Some(conflicted) = self.get_conflicted_sequencer_commit(working_tree_state) {
                commits.push(conflicted);
            }
        }
        self.get_hydrated_todo_commits(commits, true)
    }

    fn get_hydrated_todo_commits(
        &self,
        todo_commits: Vec<CommitItem>,
        _todo_file_has_short_hashes: bool,
    ) -> GitResult<Vec<CommitItem>> {
        if todo_commits.is_empty() {
            return Ok(Vec::new());
        }

        let commit_hashes: Vec<String> = todo_commits
            .iter()
            .filter_map(|c| {
                if c.oid.is_empty() {
                    None
                } else {
                    Some(c.oid.clone())
                }
            })
            .collect();

        if commit_hashes.is_empty() {
            return Ok(todo_commits);
        }

        let cmd = GitCommandBuilder::new("show")
            .config("log.showSignature=false")
            .arg(["--no-patch", "--oneline", "--abbrev=20", PRETTY_FORMAT])
            .arg(commit_hashes.iter().map(String::as_str));

        let output = self.run_git_command(cmd)?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        let mut full_commits: HashMap<String, CommitItem> = HashMap::new();
        for line in stdout.lines() {
            if line.starts_with('+') && line.len() > 1 {
                if let Some(commit) = self.extract_commit_from_line(&line[1..], false) {
                    full_commits.insert(commit.oid.clone(), commit);
                }
            }
        }

        let find_full_commit = |hash: &str| full_commits.get(hash).cloned();

        let mut hydrated_commits = Vec::with_capacity(todo_commits.len());
        for rebasing_commit in todo_commits {
            if rebasing_commit.oid.is_empty() {
                hydrated_commits.push(rebasing_commit);
            } else if let Some(mut commit) = find_full_commit(&rebasing_commit.oid) {
                commit.todo_action = rebasing_commit.todo_action;
                commit.todo_action_flag = rebasing_commit.todo_action_flag;
                commit.status = rebasing_commit.status;
                hydrated_commits.push(commit);
            }
        }

        Ok(hydrated_commits)
    }

    fn run_git_command(&self, cmd: GitCommandBuilder) -> GitResult<std::process::Output> {
        crate::git_builder_output(&self.repo_path, cmd)
    }

    fn get_rebasing_commits(&self, add_conflicting_commit: bool) -> Vec<CommitItem> {
        let todo_path = self
            .repo_path
            .join(".git")
            .join("rebase-merge")
            .join("git-rebase-todo");
        let bytes_content = match std::fs::read(&todo_path) {
            Ok(b) => b,
            Err(_) => return Vec::new(),
        };

        let mut commits = Vec::new();

        // Parse todos - simplified parsing
        let content = String::from_utf8_lossy(&bytes_content);
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Simple parsing: "pick <hash> <short_hash> <message>"
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }

            let command = parts[0];
            let hash = parts[1];
            let msg = if parts.len() > 2 {
                parts[2..].join(" ")
            } else {
                String::new()
            };

            commits.insert(
                0,
                CommitItem {
                    oid: hash.to_string(),
                    summary: msg,
                    status: CommitStatus::Rebasing,
                    todo_action: Self::parse_todo_command(command),
                    ..Default::default()
                },
            );
        }

        // Add conflicted commit if needed
        if add_conflicting_commit {
            if let Some(conflicted) = self.get_conflicted_commit() {
                commits.insert(0, conflicted);
            }
        }

        commits
    }

    fn parse_todo_command(cmd: &str) -> CommitTodoAction {
        match cmd {
            "pick" | "p" => CommitTodoAction::Pick,
            "reword" | "r" => CommitTodoAction::Reword,
            "edit" | "e" => CommitTodoAction::Edit,
            "squash" | "s" => CommitTodoAction::Squash,
            "fixup" | "f" => CommitTodoAction::Fixup,
            "drop" | "d" => CommitTodoAction::Drop,
            "merge" | "m" => CommitTodoAction::Merge,
            _ => CommitTodoAction::None,
        }
    }

    fn get_conflicted_commit(&self) -> Option<CommitItem> {
        let done_path = self
            .repo_path
            .join(".git")
            .join("rebase-merge")
            .join("done");
        let bytes_content = match std::fs::read(&done_path) {
            Ok(b) => b,
            Err(_) => return None,
        };

        let content = String::from_utf8_lossy(&bytes_content);
        let lines: Vec<&str> = content.lines().collect();

        if lines.is_empty() {
            return None;
        }

        let last_line = lines[lines.len() - 1].trim();
        let parts: Vec<&str> = last_line.split_whitespace().collect();
        if parts.len() < 2 {
            return None;
        }

        let hash = parts[1].to_string();

        Some(CommitItem {
            oid: hash,
            status: CommitStatus::Conflicted,
            ..Default::default()
        })
    }

    fn get_sequencer_commits(&self) -> Vec<CommitItem> {
        let todo_path = self.repo_path.join(".git").join("sequencer").join("todo");
        let bytes_content = match std::fs::read(&todo_path) {
            Ok(b) => b,
            Err(_) => return Vec::new(),
        };

        let content = String::from_utf8_lossy(&bytes_content);
        let mut commits = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }

            commits.insert(
                0,
                CommitItem {
                    oid: parts[1].to_string(),
                    summary: if parts.len() > 2 {
                        parts[2..].join(" ")
                    } else {
                        String::new()
                    },
                    status: CommitStatus::CherryPickingOrReverting,
                    todo_action: CommitTodoAction::Pick,
                    ..Default::default()
                },
            );
        }

        commits
    }

    fn get_conflicted_sequencer_commit(
        &self,
        working_tree_state: WorkingTreeState,
    ) -> Option<CommitItem> {
        let (sha_file, action) = if working_tree_state.cherry_picking {
            ("CHERRY_PICK_HEAD", CommitTodoAction::Pick)
        } else if working_tree_state.reverting {
            ("REVERT_HEAD", CommitTodoAction::Revert)
        } else {
            return None;
        };

        let sha_path = self.repo_path.join(".git").join(sha_file);
        let bytes_content = match std::fs::read(&sha_path) {
            Ok(b) => b,
            Err(_) => return None,
        };

        let content = String::from_utf8_lossy(&bytes_content);
        let first_line = content.lines().next()?;

        Some(CommitItem {
            oid: first_line.trim().to_string(),
            status: CommitStatus::Conflicted,
            todo_action: action,
            ..Default::default()
        })
    }

    fn get_reachable_hashes(
        &self,
        ref_name: &str,
        not_ref_names: &[String],
    ) -> HashMap<String, bool> {
        let mut args: Vec<OsString> = vec![ref_name.into()];
        for name in not_ref_names {
            args.push(format!("^{}", name).into());
        }

        let cmd = GitCommandBuilder::new("rev-list").arg(args);

        match crate::git_builder_output(&self.repo_path, cmd) {
            Ok(output) => String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| (line.trim().to_string(), true))
                .collect(),
            Err(_) => HashMap::new(),
        }
    }

    fn get_log_cmd(&self, opts: GetCommitsOptions) -> GitCommandBuilder {
        let mut ref_spec = opts.ref_name.clone();
        if let Some(ref divergence_from) = opts.ref_to_show_divergence_from {
            ref_spec = format!("{}...{}", ref_spec, divergence_from);
        }

        let mut cmd = GitCommandBuilder::new("log")
            .arg([&ref_spec])
            .arg(["--oneline"])
            .arg([PRETTY_FORMAT])
            .arg(["--abbrev=40"])
            .arg(["--no-show-signature"]);

        if opts.all {
            cmd = cmd.arg(["--all"]);
        }

        if let Some(ref author) = opts.filter_author {
            cmd = cmd.arg([format!("--author={}", author)]);
        }

        if opts.limit {
            cmd = cmd.arg(["-300"]);
        }

        if let Some(ref path) = opts.filter_path {
            cmd = cmd.arg(["--follow", "--name-status", "--", path]);
        }

        if opts.ref_to_show_divergence_from.is_some() {
            cmd = cmd.arg(["--left-right"]);
        }

        cmd
    }

    fn extract_commit_from_line(&self, line: &str, show_divergence: bool) -> Option<CommitItem> {
        extract_commit_from_line_static(line, show_divergence)
    }
}

fn load_commits_from_cmd(
    cmd: &GitCommandBuilder,
    filter_path: Option<&str>,
) -> GitResult<Vec<CommitItem>> {
    let output = crate::git_builder_output(Path::new("."), cmd.clone())?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut commits = Vec::new();
    let mut current_commit: Option<CommitItem> = None;
    let mut filter_paths: Vec<String> = Vec::new();
    let mut pool: HashMap<String, String> = HashMap::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('+') && line.len() > 1 {
            // Finish previous commit
            if let Some(mut commit) = current_commit.take() {
                if !filter_paths.is_empty() && filter_path.is_some() {
                    commit.filter_paths = filter_paths
                        .iter()
                        .filter(|p| !p.starts_with(filter_path.unwrap()))
                        .map(|p| pool.entry(p.clone()).or_insert(p.clone()).clone().into())
                        .collect();
                }
                commits.push(commit);
            }

            // Parse new commit
            current_commit = extract_commit_from_line_static(&line[1..], false);
            filter_paths.clear();
        } else if current_commit.is_some() && filter_path.is_some() {
            // Handle name-status output
            let fields: Vec<&str> = line.splitn(2, '\t').collect();
            if fields.len() > 1 {
                filter_paths.push(fields[1].to_string());
            }
        }
    }

    // Finish last commit
    if let Some(mut commit) = current_commit {
        if !filter_paths.is_empty() && filter_path.is_some() {
            commit.filter_paths = filter_paths
                .iter()
                .filter(|p| !p.starts_with(filter_path.unwrap()))
                .map(|p| pool.entry(p.clone()).or_insert(p.clone()).clone().into())
                .collect();
        }
        commits.push(commit);
    }

    Ok(commits)
}

fn extract_commit_from_line_static(line: &str, show_divergence: bool) -> Option<CommitItem> {
    let split: Vec<&str> = line.splitn(8, '\x00').collect();

    if split.len() < 7 {
        return None;
    }

    let hash = split[0].to_string();
    let unix_timestamp: i64 = split[1].parse().unwrap_or(0);
    let author_name = split[2].to_string();
    let author_email = split[3].to_string();
    let parent_hashes = split[4].to_string();

    let divergence = if show_divergence {
        if split[5] == "<" {
            CommitDivergence::Left
        } else {
            CommitDivergence::Right
        }
    } else {
        CommitDivergence::None
    };

    let extra_info = split[6].trim().to_string();
    let message = if split.len() > 7 {
        split[7].to_string()
    } else {
        String::new()
    };

    let mut tags = Vec::new();

    if !extra_info.is_empty() {
        let re = Regex::new(r"tag: ([^,]+)").ok();
        if let Some(re) = re {
            for cap in re.captures_iter(&extra_info) {
                if let Some(tag) = cap.get(1) {
                    tags.push(tag.as_str().to_string());
                }
            }
        }
    }

    let parents: Vec<String> = if parent_hashes.is_empty() {
        Vec::new()
    } else {
        parent_hashes.split(' ').map(String::from).collect()
    };

    Some(CommitItem {
        oid: hash.clone(),
        short_oid: hash[..7.min(hash.len())].to_string(),
        summary: message,
        tags,
        extra_info: format!("({})", extra_info),
        author_name,
        author_email,
        unix_timestamp,
        parents,
        status: CommitStatus::default(),
        todo_action: CommitTodoAction::None,
        todo_action_flag: String::new(),
        divergence,
        filter_paths: Vec::new(),
        changed_files: Vec::new(),
        diff: Default::default(),
    })
}

fn set_commit_statuses(unmerged_commit_hashes: &HashMap<String, bool>, commits: &mut [CommitItem]) {
    for commit in commits.iter_mut() {
        if commit.is_todo() {
            continue;
        }

        let hash = &commit.oid;
        let is_unmerged =
            unmerged_commit_hashes.is_empty() || unmerged_commit_hashes.contains_key(hash);

        if is_unmerged {
            commit.status = CommitStatus::Pushed;
        } else {
            commit.status = CommitStatus::Merged;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_commit_from_line() {
        let line = "abc123def4567890\x00123456789\x00Test Author\x00test@example.com\x00parent1 parent2\x00<\x00(HEAD -> master, tag: v1.0.0)\x00Test commit message";

        let commit = extract_commit_from_line_static(line, true).unwrap();

        assert_eq!(commit.oid, "abc123def4567890");
        assert_eq!(commit.author_name, "Test Author");
        assert_eq!(commit.author_email, "test@example.com");
        assert_eq!(commit.parents, vec!["parent1", "parent2"]);
        assert_eq!(commit.divergence, CommitDivergence::Left);
        assert!(commit.tags.contains(&"v1.0.0".to_string()));
        assert_eq!(commit.summary, "Test commit message");
    }

    #[test]
    fn test_parse_todo_command() {
        assert_eq!(
            CommitLoader::parse_todo_command("pick"),
            CommitTodoAction::Pick
        );
        assert_eq!(
            CommitLoader::parse_todo_command("reword"),
            CommitTodoAction::Reword
        );
        assert_eq!(
            CommitLoader::parse_todo_command("edit"),
            CommitTodoAction::Edit
        );
        assert_eq!(
            CommitLoader::parse_todo_command("squash"),
            CommitTodoAction::Squash
        );
        assert_eq!(
            CommitLoader::parse_todo_command("fixup"),
            CommitTodoAction::Fixup
        );
        assert_eq!(
            CommitLoader::parse_todo_command("drop"),
            CommitTodoAction::Drop
        );
        assert_eq!(
            CommitLoader::parse_todo_command("unknown"),
            CommitTodoAction::None
        );
    }
}
