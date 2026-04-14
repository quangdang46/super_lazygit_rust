// Ported from ./references/lazygit-master/pkg/gui/filetree/file_node.go

use crate::filetree::node::Node;

#[derive(Clone)]
pub struct File {
    pub path: String,
    pub name: String,
    pub short_status: String,
    pub tracked: bool,
    pub has_staged_changes: bool,
    pub has_merge_conflicts: bool,
    pub has_inline_merge_conflicts: bool,
    pub has_unstaged_changes: bool,
    pub previous_path: Option<String>,
}

pub struct FileNode {
    node: Node<File>,
}

impl FileNode {
    pub fn new(node: Option<Node<File>>) -> Option<Self> {
        node.map(|n| Self { node: n })
    }

    pub fn raw(&self) -> &Node<File> {
        &self.node
    }

    pub fn get_has_unstaged_changes(&self) -> bool {
        self.node.some_file(|file| file.has_unstaged_changes)
    }

    pub fn get_has_staged_or_tracked_changes(&self) -> bool {
        if !self.get_has_staged_changes() {
            self.node.some_file(|file| file.tracked)
        } else {
            true
        }
    }

    pub fn get_has_staged_changes(&self) -> bool {
        self.node.some_file(|file| file.has_staged_changes)
    }

    pub fn get_has_inline_merge_conflicts(&self) -> bool {
        self.node.some_file(|file| file.has_inline_merge_conflicts)
    }

    pub fn get_is_tracked(&self) -> bool {
        self.node.some_file(|file| file.tracked)
    }

    pub fn get_is_file(&self) -> bool {
        self.node.is_file()
    }

    pub fn get_previous_path(&self) -> Option<&str> {
        self.node
            .get_file()
            .map(|f| f.previous_path.as_deref().unwrap_or(""))
    }
}

impl From<Node<File>> for FileNode {
    fn from(node: Node<File>) -> Self {
        Self { node }
    }
}
