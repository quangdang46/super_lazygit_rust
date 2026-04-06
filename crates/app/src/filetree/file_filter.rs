// Ported from ./references/lazygit-master/pkg/gui/filetree/file_filter.go

pub struct FilePathSource {
    files: Vec<File>,
}

impl FilePathSource {
    pub fn string_at(&self, _i: usize) -> String {
        String::new()
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }
}

pub struct CommitFilePathSource {
    files: Vec<CommitFile>,
}

impl CommitFilePathSource {
    pub fn string_at(&self, _i: usize) -> String {
        String::new()
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }
}

pub struct File;
pub struct CommitFile;
