//! Paths management for integration tests.
//!
//! Provides convenient struct for easily getting directories within the test directory.

use std::path::PathBuf;

/// Convenience struct for easily getting directories within our test directory.
/// We have one test directory for each test, found in test/_results.
#[derive(Debug, Clone)]
pub struct Paths {
    root: PathBuf,
}

impl Paths {
    /// Creates a new Paths instance with the given root directory.
    pub fn new(root: PathBuf) -> Self {
        Paths { root }
    }

    /// Returns the root directory path.
    pub fn root(&self) -> &PathBuf {
        &self.root
    }

    /// When a test first runs, it's situated in a repo called 'repo' within this
    /// directory. In its setup step, the test is allowed to create other repos
    /// alongside the 'repo' repo in this directory, for example, creating remotes
    /// or repos to add as submodules.
    pub fn actual(&self) -> PathBuf {
        self.root.join("actual")
    }

    /// This is the 'repo' directory within the 'actual' directory,
    /// where a lazygit test will start within.
    pub fn actual_repo(&self) -> PathBuf {
        self.actual().join("repo")
    }

    /// Returns the config directory path.
    pub fn config(&self) -> PathBuf {
        self.root.join("used_config")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paths_structure() {
        let paths = Paths::new(PathBuf::from("/test/_results/my_test"));

        assert_eq!(paths.root(), &PathBuf::from("/test/_results/my_test"));
        assert_eq!(
            paths.actual(),
            PathBuf::from("/test/_results/my_test/actual")
        );
        assert_eq!(
            paths.actual_repo(),
            PathBuf::from("/test/_results/my_test/actual/repo")
        );
        assert_eq!(
            paths.config(),
            PathBuf::from("/test/_results/my_test/used_config")
        );
    }
}
