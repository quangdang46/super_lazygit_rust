// Ported from ./references/lazygit-master/pkg/gui/filetree/collapsed_paths.go

pub struct CollapsedPaths {
    collapsed_paths: Vec<String>,
}

impl CollapsedPaths {
    pub fn new() -> Self {
        Self {
            collapsed_paths: Vec::new(),
        }
    }

    pub fn expand_to_path(&mut self, _path: String) {}

    pub fn is_collapsed(&self, _path: &String) -> bool {
        false
    }

    pub fn collapse(&mut self, _path: String) {}

    pub fn toggle_collapsed(&mut self, _path: String) {}

    pub fn expand_all(&mut self) {}
}
