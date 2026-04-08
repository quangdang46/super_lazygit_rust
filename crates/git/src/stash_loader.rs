use regex::Regex;

use super_lazygit_core::state::StashItem;

use crate::{git_builder_output, GitCommandBuilder, GitResult};

pub struct StashLoader {
    repo_path: std::path::PathBuf,
}

impl StashLoader {
    pub fn new(repo_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
        }
    }

    /// Get stash entries, optionally filtered by path
    pub fn get_stash_entries(&self, filter_path: Option<&str>) -> GitResult<Vec<StashItem>> {
        if filter_path.is_none() {
            return self.get_unfiltered_stash_entries();
        }

        let filter_path = filter_path.unwrap();
        let cmd =
            GitCommandBuilder::new("stash").arg(["list", "--name-only", "--pretty=%gd:%H|%ct|%gs"]);

        let output = git_builder_output(&self.repo_path, cmd)?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        let mut stash_entries = Vec::new();
        let lines: Vec<&str> = stdout.lines().collect();
        let re = Regex::new(r"^stash@\{(\d+)\}:(.*)$").unwrap();
        let is_a_stash = |line: &str| line.starts_with("stash@{");

        for i in 0..lines.len() {
            let line = lines[i];
            if let Some(caps) = re.captures(line) {
                let idx: usize = caps[1].parse().unwrap_or(0);
                let remainder = caps.get(2).map_or("", |m| m.as_str());

                let current_stash = stash_entry_from_line(remainder, idx);

                // Check subsequent lines for the filter path
                let mut j = i + 1;
                while j < lines.len() && !is_a_stash(lines[j]) {
                    if lines[j].starts_with(filter_path) {
                        stash_entries.push(current_stash);
                        break;
                    }
                    j += 1;
                }
            }
        }

        if stash_entries.is_empty() {
            return self.get_unfiltered_stash_entries();
        }

        Ok(stash_entries)
    }

    fn get_unfiltered_stash_entries(&self) -> GitResult<Vec<StashItem>> {
        let cmd = GitCommandBuilder::new("stash").arg(["list", "-z", "--pretty=%H|%ct|%gs"]);

        let output = git_builder_output(&self.repo_path, cmd)?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Split by null character
        let entries: Vec<StashItem> = stdout
            .split('\0')
            .filter(|line| !line.is_empty())
            .enumerate()
            .map(|(index, line)| stash_entry_from_line(line, index))
            .collect();

        Ok(entries)
    }
}

fn stash_entry_from_line(line: &str, index: usize) -> StashItem {
    let mut model = StashItem {
        index,
        recency: String::new(),
        name: String::new(),
        hash: String::new(),
        stash_ref: format!("stash@{{{}}}", index),
        label: String::new(),
        changed_files: Vec::new(),
    };

    // Format: "hash|unix_timestamp|message" or just "message" for filtered
    if let Some((hash, rest)) = line.split_once('|') {
        model.hash = hash.to_string();

        if let Some((tstr, msg)) = rest.split_once('|') {
            if let Ok(t) = tstr.parse::<i64>() {
                model.name = msg.to_string();
                model.recency = unix_to_time_ago(t);
            } else {
                model.name = rest.to_string();
            }
        } else {
            model.name = rest.to_string();
        }
    } else {
        model.name = line.to_string();
    }

    model.label = model.name.clone();

    model
}

fn unix_to_time_ago(unix_timestamp: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let diff = now - unix_timestamp;

    if diff < 60 {
        return "just now".to_string();
    }

    let minutes = diff / 60;
    if minutes < 60 {
        return format!("{} minutes ago", minutes);
    }

    let hours = diff / 3600;
    if hours < 24 {
        return format!("{} hours ago", hours);
    }

    let days = diff / 86400;
    if days < 30 {
        return format!("{} days ago", days);
    }

    let weeks = diff / 604800;
    if weeks < 52 {
        return format!("{} weeks ago", weeks);
    }

    let years = diff / 31536000;
    format!("{} years ago", years)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stash_entry_from_line() {
        let entry =
            stash_entry_from_line("abc123|1700000000|WIP on main: 1234567 commit message", 0);

        assert_eq!(entry.index, 0);
        assert_eq!(entry.hash, "abc123");
        assert_eq!(entry.name, "WIP on main: 1234567 commit message");
        assert_eq!(entry.stash_ref, "stash@{0}");
    }

    #[test]
    fn test_stash_entry_from_line_simple() {
        let entry = stash_entry_from_line("simple stash message", 5);

        assert_eq!(entry.index, 5);
        assert_eq!(entry.name, "simple stash message");
        assert_eq!(entry.stash_ref, "stash@{5}");
    }

    #[test]
    fn test_unix_to_time_ago() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        assert_eq!(unix_to_time_ago(now), "just now");
        assert_eq!(unix_to_time_ago(now - 30), "just now");
        assert_eq!(unix_to_time_ago(now - 120), "2 minutes ago");
        assert_eq!(unix_to_time_ago(now - 3600), "1 hours ago");
        assert_eq!(unix_to_time_ago(now - 7200), "2 hours ago");
        assert_eq!(unix_to_time_ago(now - 86400), "1 days ago");
    }
}
