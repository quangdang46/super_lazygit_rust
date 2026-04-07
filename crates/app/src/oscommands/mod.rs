//! OS commands module.
//!
//! This module provides command execution functionality for lazygit.
//!
//! # Architecture
//!
//! The module is split into several files to avoid circular dependencies:
//! - `traits.rs`: Defines `CmdRunner` trait and related types
//! - `cmd_obj.rs`: The `CmdObj` struct that represents a command
//! - `cmd_obj_runner.rs`: The default and fake runners
//! - `mod.rs`: The main `OSCommand` struct that ties everything together
//!
//! The circular dependency is broken by having `CmdObj` be a plain data struct
//! (no runner reference), and having the runner implementations be separate.

mod cmd_obj;
mod cmd_obj_runner;
mod traits;

pub use cmd_obj::CmdObj;
pub use cmd_obj_runner::{CmdLogger, CmdObjRunner, FakeCmdObjRunner, GuiIo, ThreadSafeCmdRunner};
pub use traits::{CmdRunner, CredentialStrategy, RunError, Task};

use std::path::Path;
use std::process::Command;
use std::sync::Arc;

/// Platform-specific configuration.
#[derive(Debug, Clone)]
pub struct Platform {
    /// The OS name (e.g., "linux", "windows", "darwin").
    pub os: String,
    /// The shell to use.
    pub shell: String,
    /// Arguments for the shell.
    pub shell_arg: String,
    /// Prefix for shell functions file.
    pub prefix_for_shell_functions_file: String,
    /// The open command.
    pub open_command: String,
    /// The open link command.
    pub open_link_command: String,
}

impl Default for Platform {
    fn default() -> Self {
        if cfg!(windows) {
            Platform {
                os: "windows".to_string(),
                shell: "cmd".to_string(),
                shell_arg: "/C".to_string(),
                prefix_for_shell_functions_file: String::new(),
                open_command: "start".to_string(),
                open_link_command: "start".to_string(),
            }
        } else {
            Platform {
                os: "linux".to_string(),
                shell: "sh".to_string(),
                shell_arg: "-c".to_string(),
                prefix_for_shell_functions_file: ". ".to_string(),
                open_command: "xdg-open".to_string(),
                open_link_command: "xdg-open".to_string(),
            }
        }
    }
}

/// Command builder for creating new commands.
pub struct CmdObjBuilder {
    runner: Arc<dyn CmdRunner>,
    platform: Platform,
}

impl CmdObjBuilder {
    /// Create a new command with the given arguments.
    pub fn new(&self, args: Vec<&str>) -> CmdObj {
        let mut cmd = Command::new(args[0]);
        for arg in &args[1..] {
            cmd.arg(*arg);
        }
        CmdObj::new(cmd)
    }

    /// Create a shell command.
    pub fn new_shell(&self, command: &str, shell_functions_file: Option<&str>) -> CmdObj {
        let mut full_command = String::new();

        if let Some(file) = shell_functions_file {
            if !file.is_empty() {
                full_command.push_str(&self.platform.prefix_for_shell_functions_file);
                full_command.push_str(file);
                full_command.push('\n');
            }
        }

        full_command.push_str(command);

        let quoted = self.quote(&full_command);
        let mut cmd = Command::new(&self.platform.shell);
        cmd.arg(&self.platform.shell_arg);
        cmd.arg(quoted);

        CmdObj::new(cmd)
    }

    /// Quote a string for shell usage.
    pub fn quote(&self, message: &str) -> String {
        if self.platform.os == "windows" {
            format!("\"{}\"", message.replace("\"", "\"^\"^\""))
        } else {
            let escaped = message
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('$', "\\$")
                .replace('`', "\\`");
            format!("\"{}\"", escaped)
        }
    }
}

/// Main OS command executor.
pub struct OSCommand {
    /// Platform configuration.
    platform: Platform,
    /// Command builder.
    cmd: CmdObjBuilder,
    /// Log for command recording.
    log: Arc<dyn CmdLogger>,
    /// Runner for executing commands.
    runner: Arc<dyn CmdRunner>,
    /// Temporary directory.
    temp_dir: String,
}

impl OSCommand {
    /// Create a new OS command executor.
    pub fn new(log: Arc<dyn CmdLogger>, temp_dir: &str) -> Self {
        let platform = Platform::default();
        let runner = Arc::new(CmdObjRunner::new(log.clone())) as Arc<dyn CmdRunner>;
        let cmd = CmdObjBuilder {
            runner: runner.clone(),
            platform: platform.clone(),
        };

        OSCommand {
            platform,
            cmd,
            log,
            runner,
            temp_dir: temp_dir.to_string(),
        }
    }

    /// Create a new shell command.
    pub fn new_shell(&self, command: &str) -> CmdObj {
        self.cmd.new_shell(command, None)
    }

    /// Quote a string.
    pub fn quote(&self, message: &str) -> String {
        self.cmd.quote(message)
    }

    /// Get the temporary directory.
    pub fn get_temp_dir(&self) -> &str {
        &self.temp_dir
    }

    /// Get an environment variable.
    pub fn getenv(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }

    /// Remove a file.
    pub fn remove_file(&self, path: &Path) -> std::io::Result<()> {
        std::fs::remove_file(path)
    }

    /// Check if a file exists.
    pub fn file_exists(&self, path: &Path) -> bool {
        path.exists()
    }

    /// Log a command.
    pub fn log_command(&self, cmd: &str, command_line: bool) {
        self.log.log_command(cmd, command_line);
    }

    /// Run a command.
    pub fn run(&self, cmd_obj: CmdObj) -> Result<(), RunError> {
        self.runner.run(cmd_obj.cmd)
    }

    /// Run a command and capture output.
    pub fn run_with_output(&self, cmd_obj: CmdObj) -> Result<String, RunError> {
        self.runner.run_with_output(cmd_obj.cmd)
    }

    /// Run a command and capture both outputs.
    pub fn run_with_outputs(&self, cmd_obj: CmdObj) -> Result<(String, String), RunError> {
        self.runner.run_with_outputs(cmd_obj.cmd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn test_cmd_builder_new() {
        let logger = Arc::new(MockCmdLogger::new());
        let os_cmd = OSCommand::new(logger, "/tmp");
        let cmd = os_cmd.new_shell("echo hello");
        assert!(cmd.to_string().contains("echo"));
    }

    #[test]
    fn test_quote_unix() {
        let logger = Arc::new(MockCmdLogger::new());
        let os_cmd = OSCommand::new(logger, "/tmp");
        let quoted = os_cmd.quote("hello world");
        assert_eq!(quoted, "\"hello world\"");
    }
}
