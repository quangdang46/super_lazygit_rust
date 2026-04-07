//! Command object runner implementation.
//!
//! The default runner that executes commands.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::Arc;

use super::traits::{CmdRunner, RunError};

/// Logger trait for command execution logging.
pub trait CmdLogger: Send + Sync {
    /// Log a command.
    fn log_command(&self, cmd: &str, command_line: bool);
}

/// GUI IO trait for user interaction.
pub trait GuiIo: Send + Sync {
    /// Log a command for display.
    fn log_command_fn(&self, cmd: &str, command_line: bool);
}

/// The default command object runner.
pub struct CmdObjRunner {
    /// Log for recording command execution.
    log: Arc<dyn CmdLogger>,
}

impl CmdObjRunner {
    /// Create a new command runner.
    pub fn new(log: Arc<dyn CmdLogger>) -> Self {
        CmdObjRunner { log }
    }
}

impl CmdRunner for CmdObjRunner {
    fn run(&self, mut cmd: Command) -> Result<(), RunError> {
        let output = cmd.output().map_err(|e| RunError::new(e.to_string()))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RunError::new(stderr.to_string()));
        }
        Ok(())
    }

    fn run_with_output(&self, mut cmd: Command) -> Result<String, RunError> {
        let output = cmd.output().map_err(|e| RunError::new(e.to_string()))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RunError::new(stderr.to_string()));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn run_with_outputs(&self, mut cmd: Command) -> Result<(String, String), RunError> {
        let output = cmd.output().map_err(|e| RunError::new(e.to_string()))?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if !output.status.success() {
            return Err(RunError::new(stderr));
        }
        Ok((stdout, stderr))
    }
}

/// Thread-safe wrapper for CmdRunner.
pub struct ThreadSafeCmdRunner(Arc<dyn CmdRunner>);

impl ThreadSafeCmdRunner {
    pub fn new(runner: impl CmdRunner + 'static) -> Self {
        ThreadSafeCmdRunner(Arc::new(runner))
    }
}

impl CmdRunner for ThreadSafeCmdRunner {
    fn run(&self, cmd: Command) -> Result<(), RunError> {
        self.0.run(cmd)
    }

    fn run_with_output(&self, cmd: Command) -> Result<String, RunError> {
        self.0.run_with_output(cmd)
    }

    fn run_with_outputs(&self, cmd: Command) -> Result<(String, String), RunError> {
        self.0.run_with_outputs(cmd)
    }
}

/// Fake runner for testing.
pub struct FakeCmdObjRunner;

impl FakeCmdObjRunner {
    pub fn new() -> Self {
        FakeCmdObjRunner
    }
}

impl Default for FakeCmdObjRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl CmdRunner for FakeCmdObjRunner {
    fn run(&self, _cmd: Command) -> Result<(), RunError> {
        Ok(())
    }

    fn run_with_output(&self, _cmd: Command) -> Result<String, RunError> {
        Ok(String::new())
    }

    fn run_with_outputs(&self, _cmd: Command) -> Result<(String, String), RunError> {
        Ok((String::new(), String::new()))
    }
}
