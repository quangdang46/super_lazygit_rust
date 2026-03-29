use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct WorkspaceRegistry {
    root: Option<PathBuf>,
}

impl WorkspaceRegistry {
    #[must_use]
    pub fn new(root: Option<PathBuf>) -> Self {
        Self { root }
    }

    #[must_use]
    pub fn root(&self) -> Option<&PathBuf> {
        self.root.as_ref()
    }
}
