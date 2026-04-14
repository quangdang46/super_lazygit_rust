use std::path::PathBuf;

use super_lazygit_core::RepoId;

use crate::version::GitVersion;

#[derive(Debug, Clone)]
pub struct GitContext {
    pub version: GitVersion,
    pub repo_id: RepoId,
}

impl GitContext {
    #[must_use]
    pub fn new(repo_id: RepoId, version: GitVersion) -> Self {
        Self { version, repo_id }
    }

    #[must_use]
    pub fn repo_path(&self) -> PathBuf {
        PathBuf::from(&self.repo_id.0)
    }
}
