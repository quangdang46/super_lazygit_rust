// Ported from ./references/lazygit-master/pkg/gui/modes/marked_base_commit/marked_base_commit.go

#[derive(Clone, Default)]
pub struct MarkedBaseCommit {
    pub hash: String,
}

impl MarkedBaseCommit {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn active(&self) -> bool {
        !self.hash.is_empty()
    }

    pub fn reset(&mut self) {
        self.hash = String::new();
    }

    pub fn set_hash(&mut self, hash: &str) {
        self.hash = hash.to_string();
    }

    pub fn get_hash(&self) -> &str {
        &self.hash
    }
}
