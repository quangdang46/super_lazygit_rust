use crate::integration_test::TestShell;

pub const ORIGINAL_FILE_CONTENT: &str = "This\nIs\nThe\nOriginal\nFile\n";

pub const FIRST_CHANGE_FILE_CONTENT: &str = "This\nIs\nThe\nFirst Change\nFile\n";

pub const SECOND_CHANGE_FILE_CONTENT: &str = "This\nIs\nThe\nSecond Change\nFile\n";

pub fn merge_conflicts_setup(_shell: &TestShell) {}

pub fn create_merge_conflict_file(_shell: &TestShell) {}

pub fn create_merge_commit(_shell: &TestShell) {}

pub fn create_merge_conflict_files(_shell: &TestShell) {}
