//! Git command object builder.
//!
//! This module provides a command builder specifically for git commands,
//! adding git-specific environment variables like GIT_OPTIONAL_LOCKS=0.

use super::{CmdObj, CmdObjBuilder};
use std::sync::Arc;

/// The default git environment variable to disable optional locks.
const GIT_OPTIONAL_LOCKS: &str = "GIT_OPTIONAL_LOCKS=0";

/// Git-specific command object builder.
///
/// This builder wraps the standard `CmdObjBuilder` and automatically adds
/// the `GIT_OPTIONAL_LOCKS=0` environment variable to all commands.
pub struct GitCmdObjBuilder {
    /// The inner command builder.
    inner: CmdObjBuilder,
}

impl GitCmdObjBuilder {
    /// Create a new git command builder.
    pub fn new(builder: CmdObjBuilder) -> Self {
        Self { inner: builder }
    }

    /// Create a new command with the given arguments.
    ///
    /// Automatically adds `GIT_OPTIONAL_LOCKS=0` to the command.
    pub fn new_cmd(&self, args: Vec<&str>) -> CmdObj {
        self.inner.new(args).add_env_vars(&[GIT_OPTIONAL_LOCKS])
    }

    /// Create a new shell command.
    ///
    /// Automatically adds `GIT_OPTIONAL_LOCKS=0` to the command.
    pub fn new_shell(&self, command: &str, shell_functions_file: Option<&str>) -> CmdObj {
        self.inner
            .new_shell(command, shell_functions_file)
            .add_env_vars(&[GIT_OPTIONAL_LOCKS])
    }

    /// Quote a string for shell usage.
    pub fn quote(&self, message: &str) -> String {
        self.inner.quote(message)
    }
}

/// A thread-safe reference to a git command builder.
pub type SharedGitCmdObjBuilder = Arc<GitCmdObjBuilder>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oscommands::{CmdLogger, OSCommand};
    use std::sync::Mutex;

    struct MockCmdLogger {
        logs: Mutex<Vec<String>>,
    }

    impl MockCmdLogger {
        fn new() -> Self {
            MockCmdLogger {
                logs: Mutex::new(Vec::new()),
            }
        }
    }

    impl CmdLogger for MockCmdLogger {
        fn log_command(&self, cmd: &str, _command_line: bool) {
            self.logs.lock().unwrap().push(cmd.to_string());
        }
    }

    #[test]
    fn test_git_cmd_builder_adds_optional_locks() {
        let logger = Arc::new(MockCmdLogger::new());
        let os_cmd = OSCommand::new(logger, "/tmp", None, None);
        let cmd = os_cmd.new_shell("echo hello");
        assert!(cmd.to_string().contains("GIT_OPTIONAL_LOCKS=0"));
    }
}
