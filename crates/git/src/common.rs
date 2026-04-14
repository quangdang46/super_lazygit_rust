use std::path::PathBuf;

use super_lazygit_core::RepoId;

#[derive(Debug, Clone)]
pub struct GitCommon {
    pub repo_id: RepoId,
    pub repo_path: PathBuf,
}

impl GitCommon {
    #[must_use]
    pub fn new(repo_id: RepoId, repo_path: PathBuf) -> Self {
        Self { repo_id, repo_path }
    }
}
