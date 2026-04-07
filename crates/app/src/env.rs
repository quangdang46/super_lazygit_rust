use std::env;

/// Gets the GIT_DIR environment variable.
pub fn get_git_dir_env() -> Option<String> {
    env::var("GIT_DIR").ok()
}

/// Sets the GIT_DIR environment variable.
pub fn set_git_dir_env(value: &str) {
    env::set_var("GIT_DIR", value);
}

/// Gets the GIT_WORK_TREE environment variable.
pub fn get_work_tree_env() -> Option<String> {
    env::var("GIT_WORK_TREE").ok()
}

/// Sets the GIT_WORK_TREE environment variable.
pub fn set_work_tree_env(value: &str) {
    env::set_var("GIT_WORK_TREE", value);
}

/// Unsets both GIT_DIR and GIT_WORK_TREE environment variables.
pub fn unset_git_location_env_vars() {
    env::remove_var("GIT_DIR");
    env::remove_var("GIT_WORK_TREE");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_git_dir_env_not_set() {
        // In test environment, these may not be set
        let _ = get_git_dir_env();
        let _ = get_work_tree_env();
    }
}
