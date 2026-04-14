use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BisectStatus {
    Old,
    New,
    Skipped,
}

#[derive(Debug, Clone)]
pub struct BisectInfo {
    started: bool,
    start: String,
    new_term: String,
    old_term: String,
    status_map: HashMap<String, BisectStatus>,
    current: String,
}

impl BisectInfo {
    pub fn null() -> Self {
        Self {
            started: false,
            start: String::new(),
            new_term: String::new(),
            old_term: String::new(),
            status_map: HashMap::new(),
            current: String::new(),
        }
    }

    pub fn get_new_hash(&self) -> String {
        for (hash, status) in &self.status_map {
            if *status == BisectStatus::New {
                return hash.clone();
            }
        }
        String::new()
    }

    pub fn get_current_hash(&self) -> &str {
        &self.current
    }

    pub fn get_start_hash(&self) -> &str {
        &self.start
    }

    pub fn status_map(&self) -> &HashMap<String, BisectStatus> {
        &self.status_map
    }

    pub fn status(&self, commit_hash: &str) -> Option<BisectStatus> {
        self.status_map.get(commit_hash).copied()
    }

    pub fn new_term(&self) -> &str {
        &self.new_term
    }

    pub fn old_term(&self) -> &str {
        &self.old_term
    }

    pub fn started(&self) -> bool {
        self.started
    }

    pub fn bisecting(&self) -> bool {
        if !self.started() {
            return false;
        }

        if self.get_new_hash().is_empty() {
            return false;
        }

        self.status_map
            .values()
            .any(|&status| status == BisectStatus::Old)
    }
}

pub fn get_info_for_git_dir(git_dir: &Path) -> BisectInfo {
    let mut info = BisectInfo {
        started: false,
        start: String::new(),
        new_term: "bad".to_string(),
        old_term: "good".to_string(),
        status_map: HashMap::new(),
        current: String::new(),
    };

    let bisect_start_path = git_dir.join("BISECT_START");
    if !bisect_start_path.exists() {
        return info;
    }

    let Ok(start_content) = std::fs::read_to_string(&bisect_start_path) else {
        return info;
    };

    info.started = true;
    info.start = start_content.trim().to_string();

    let terms_path = git_dir.join("BISECT_TERMS");
    if let Ok(terms_content) = std::fs::read_to_string(&terms_path) {
        let split_content: Vec<&str> = terms_content.split('\n').collect();
        if !split_content.is_empty() {
            info.new_term = split_content[0].to_string();
        }
        if split_content.len() > 1 {
            info.old_term = split_content[1].to_string();
        }
    }

    let bisect_refs_dir = git_dir.join("refs").join("bisect");
    let Ok(entries) = std::fs::read_dir(&bisect_refs_dir) else {
        return info;
    };

    info.status_map = HashMap::new();
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        let path = entry.path();

        let Ok(file_content) = std::fs::read_to_string(&path) else {
            continue;
        };

        let hash = file_content.trim().to_string();

        let status = if name == info.new_term {
            BisectStatus::New
        } else if name.starts_with(&format!("{}-", info.old_term)) {
            BisectStatus::Old
        } else if name.starts_with("skipped-") {
            BisectStatus::Skipped
        } else {
            BisectStatus::Skipped
        };

        info.status_map.insert(hash, status);
    }

    let expected_rev_path = git_dir.join("BISECT_EXPECTED_REV");
    if let Ok(current_content) = std::fs::read_to_string(&expected_rev_path) {
        info.current = current_content.trim().to_string();
    }

    info
}
