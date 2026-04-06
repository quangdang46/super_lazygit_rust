// Ported from ./references/lazygit-master/pkg/gui/modes/cherrypicking/cherry_picking.go

use std::collections::HashSet;

#[derive(Clone)]
pub struct CherryPicking {
    pub cherry_picked_commits: Vec<Commit>,
    pub context_key: String,
    pub did_paste: bool,
}

#[derive(Clone)]
pub struct Commit {
    pub hash: String,
}

impl Commit {
    pub fn hash(&self) -> &str {
        &self.hash
    }
}

impl CherryPicking {
    pub fn new() -> Self {
        Self {
            cherry_picked_commits: Vec::new(),
            context_key: String::new(),
            did_paste: false,
        }
    }

    pub fn active(&self) -> bool {
        self.can_paste() && !self.did_paste
    }

    pub fn can_paste(&self) -> bool {
        !self.cherry_picked_commits.is_empty()
    }

    pub fn selected_hash_set(&self) -> HashSet<String> {
        if self.did_paste {
            return HashSet::new();
        }

        self.cherry_picked_commits
            .iter()
            .map(|c| c.hash().to_string())
            .collect()
    }

    pub fn add(&mut self, selected_commit: &Commit, commits_list: &[Commit]) {
        let mut commit_set = self.selected_hash_set();
        commit_set.insert(selected_commit.hash().to_string());

        self.update(commit_set, commits_list);
    }

    pub fn remove(&mut self, selected_commit: &Commit, commits_list: &[Commit]) {
        let mut commit_set = self.selected_hash_set();
        commit_set.remove(selected_commit.hash());

        self.update(commit_set, commits_list);
    }

    fn update(&mut self, selected_hash_set: HashSet<String>, commits_list: &[Commit]) {
        self.cherry_picked_commits = commits_list
            .iter()
            .filter(|commit| selected_hash_set.contains(commit.hash()))
            .map(|commit| commit.clone())
            .collect();
    }
}

impl Default for CherryPicking {
    fn default() -> Self {
        Self::new()
    }
}
