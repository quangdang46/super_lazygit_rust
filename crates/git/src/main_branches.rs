use std::collections::HashMap;
use std::process::Command;
use std::sync::mpsc;
use std::thread;

use std::sync::Mutex;

use super_lazygit_core::RepoId;

/// Main branches detection and management.
pub struct MainBranches {
    repo_id: RepoId,
    /// Which of the configured main branches actually exist in the repository.
    /// Full ref names, and it could be either "refs/heads/..." or "refs/remotes/origin/..."
    /// depending on which one exists for a given bare name.
    existing_main_branches: Mutex<Vec<String>>,
    previous_main_branches: Mutex<Vec<String>>,
    /// User-configured main branch names (e.g., ["main", "master"])
    configured_main_branches: Vec<String>,
}

impl MainBranches {
    /// Create a new MainBranches instance.
    ///
    /// # Arguments
    ///
    /// * `repo_id` - The repository identifier
    /// * `configured_main_branches` - The configured main branch names from user config
    #[must_use]
    pub fn new(repo_id: RepoId, configured_main_branches: Vec<String>) -> Self {
        Self {
            repo_id,
            existing_main_branches: Mutex::new(Vec::new()),
            previous_main_branches: Mutex::new(Vec::new()),
            configured_main_branches,
        }
    }

    /// Get the list of main branches that exist in the repository.
    /// This is a list of full ref names.
    pub fn get(&self) -> Vec<String> {
        let mut existing = self.existing_main_branches.lock().unwrap();
        let mut previous = self.previous_main_branches.lock().unwrap();

        if existing.is_empty() || *previous != self.configured_main_branches {
            *existing = self.determine_main_branches(&self.configured_main_branches);
            *previous = self.configured_main_branches.clone();
        }

        existing.clone()
    }

    /// Return the merge base of the given refName with the closest main branch.
    pub fn get_merge_base(&self, ref_name: &str) -> String {
        let main_branches = self.get();
        if main_branches.is_empty() {
            return String::new();
        }

        // We pass all existing main branches to the merge-base call; git will
        // return the base commit for the closest one.
        let output = Command::new("git")
            .arg("-C")
            .arg(self.repo_id.0.as_str())
            .arg("merge-base")
            .arg(ref_name)
            .args(&main_branches)
            .output();

        output
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default()
    }

    fn determine_main_branches(&self, configured_main_branches: &[String]) -> Vec<String> {
        let (tx, rx) = mpsc::channel();
        let repo_path = self.repo_id.0.clone();

        for branch_name in configured_main_branches {
            let tx = mpsc::Sender::clone(&tx);
            let repo_path = repo_path.clone();
            let branch_name = branch_name.clone();

            thread::spawn(move || {
                let result = Self::find_existing_branch(&repo_path, &branch_name);
                let _ = tx.send((branch_name, result));
            });
        }

        drop(tx);

        let mut results: HashMap<String, String> = HashMap::new();
        for (branch_name, ref_name) in rx {
            if let Some(ref_name) = ref_name {
                results.insert(branch_name, ref_name);
            }
        }

        configured_main_branches
            .iter()
            .filter_map(|name| results.get(name).cloned())
            .collect()
    }

    fn find_existing_branch(repo_path: &str, branch_name: &str) -> Option<String> {
        // Try to determine upstream of local main branch
        let upstream_output = Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .args([
                "rev-parse",
                "--symbolic-full-name",
                &format!("{branch_name}@{{u}}"),
            ])
            .output();

        if let Ok(output) = upstream_output {
            if output.status.success() {
                let ref_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !ref_name.is_empty() {
                    return Some(ref_name);
                }
            }
        }

        // If this failed, a local branch for this main branch doesn't exist or it
        // has no upstream configured. Try looking for one in the "origin" remote.
        let remote_ref = format!("refs/remotes/origin/{branch_name}");
        let verify_output = Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .args(["rev-parse", "--verify", "--quiet", &remote_ref])
            .output();

        if let Ok(output) = verify_output {
            if output.status.success() {
                return Some(remote_ref);
            }
        }

        // If this failed as well, try if we have the main branch as a local
        // branch. This covers the case where somebody is using git locally
        // for something, but never pushing anywhere.
        let local_ref = format!("refs/heads/{branch_name}");
        let verify_output = Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .args(["rev-parse", "--verify", "--quiet", &local_ref])
            .output();

        if let Ok(output) = verify_output {
            if output.status.success() {
                return Some(local_ref);
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_main_branches_empty_config() {
        let main_branches = MainBranches::new(RepoId("/tmp/nonexistent".into()), vec![]);
        assert!(main_branches.get().is_empty());
    }
}
