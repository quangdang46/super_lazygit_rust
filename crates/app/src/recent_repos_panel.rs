// Ported from ./references/lazygit-master/pkg/gui/recent_repos_panel.go

use std::path::Path;

pub fn new_recent_repos_list(recent_repos: &[String], current_repo: &str) -> Vec<String> {
    let mut new_repos = vec![current_repo.to_string()];
    for repo in recent_repos {
        if repo != current_repo {
            if let Ok(metadata) = std::fs::metadata(Path::new(repo).join(".git")) {
                if metadata.is_dir() {
                    new_repos.push(repo.clone());
                }
            }
        }
    }
    new_repos
}
