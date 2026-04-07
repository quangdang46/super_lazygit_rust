//! Traits for command execution.

use std::process::Command;

/// Credential strategy for commands that may prompt for credentials.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CredentialStrategy {
    /// Do not expect a credential request.
    #[default]
    None,
    /// Expect a credential request and prompt the user.
    Prompt,
    /// Expect a credential request and fail immediately.
    Fail,
}

/// Task identifier for concurrent task management.
#[derive(Debug, Clone, Copy, Default)]
pub struct Task;

/// Error from running a command.
#[derive(Debug, Clone)]
pub struct RunError {
    pub message: String,
}

impl RunError {
    pub fn new(message: impl Into<String>) -> Self {
        RunError { message: message.into() }
    }
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for RunError {}

/// Trait for running commands.
pub trait CmdRunner: Send + Sync {
    fn run(&self, cmd: Command) -> Result<(), RunError>;
    fn run_with_output(&self, cmd: Command) -> Result<String, RunError>;
    fn run_with_outputs(&self, cmd: Command) -> Result<(String, String), RunError>;
}
