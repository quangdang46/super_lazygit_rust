use tempfile::TempDir;

pub fn temp_repo() -> std::io::Result<TempDir> {
    tempfile::tempdir()
}
