// Ported from ./references/lazygit-master/pkg/gui/filetree/commit_file_node.go

pub struct CommitFileNode;

impl CommitFileNode {
    pub fn new(_node: &CommitFileNode) -> Option<Self> {
        None
    }

    pub fn raw(&self) -> &CommitFileNode {
        self
    }
}
