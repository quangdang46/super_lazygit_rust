// Ported from ./references/lazygit-master/pkg/gui/modes/filtering/filtering.go

#[derive(Clone, Default)]
pub struct Filtering {
    pub path: String,
    pub author: String,
    pub selected_commit_hash: String,
}

impl Filtering {
    pub fn new(path: &str, author: &str) -> Self {
        Self {
            path: path.to_string(),
            author: author.to_string(),
            selected_commit_hash: String::new(),
        }
    }

    pub fn active(&self) -> bool {
        !self.path.is_empty() || !self.author.is_empty()
    }

    pub fn reset(&mut self) {
        self.path = String::new();
        self.author = String::new();
    }

    pub fn set_path(&mut self, path: &str) {
        self.path = path.to_string();
    }

    pub fn get_path(&self) -> &str {
        &self.path
    }

    pub fn set_author(&mut self, author: &str) {
        self.author = author.to_string();
    }

    pub fn get_author(&self) -> &str {
        &self.author
    }

    pub fn set_selected_commit_hash(&mut self, hash: &str) {
        self.selected_commit_hash = hash.to_string();
    }

    pub fn get_selected_commit_hash(&self) -> &str {
        &self.selected_commit_hash
    }
}
