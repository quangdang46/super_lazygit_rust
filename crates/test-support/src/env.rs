//! Environment variables and helpers for integration tests.

use std::env;

/// Environment variable names for lazygit root directory.
pub const LAZYGIT_ROOT_DIR: &str = "LAZYGIT_ROOT_DIR";
/// Environment variable for sandbox mode.
pub const SANDBOX_ENV_VAR: &str = "SANDBOX";
/// Environment variable for test name.
pub const TEST_NAME_ENV_VAR: &str = "TEST_NAME";
/// Environment variable for waiting for debugger.
pub const WAIT_FOR_DEBUGGER_ENV_VAR: &str = "WAIT_FOR_DEBUGGER";
/// Environment variable for git config global.
pub const GIT_CONFIG_GLOBAL_ENV_VAR: &str = "GIT_CONFIG_GLOBAL";
/// Working directory environment variable (preserves symlinks).
pub const PWD: &str = "PWD";
/// Home directory environment variable.
pub const HOME: &str = "HOME";
/// Git config no global environment variable.
pub const GIT_CONFIG_NOGLOBAL: &str = "GIT_CONFIG_NOGLOBAL";
/// Path environment variable.
pub const PATH: &str = "PATH";
/// Terminal environment variable.
pub const TERM: &str = "TERM";

const TEST_DIR: &str = "test";

fn test_path(root_dir: &str) -> String {
    format!("{}/{}", root_dir, TEST_DIR)
}

fn global_git_config_path(root_dir: &str) -> String {
    format!("{}/test/global_git_config", root_dir)
}

pub fn allowed_host_environment() -> Vec<String> {
    let mut env_vars = Vec::new();

    if let Ok(path) = env::var(PATH) {
        env_vars.push(format!("{}={}", PATH, path));
    }
    if let Ok(term) = env::var(TERM) {
        env_vars.push(format!("{}={}", TERM, term));
    }

    env_vars
}

pub fn new_test_environment(root_dir: &str) -> Vec<String> {
    let mut env = allowed_host_environment();

    env.push(format!("{}={}", HOME, test_path(root_dir)));
    env.push(format!(
        "{}={}",
        GIT_CONFIG_GLOBAL_ENV_VAR,
        global_git_config_path(root_dir)
    ));

    env
}
