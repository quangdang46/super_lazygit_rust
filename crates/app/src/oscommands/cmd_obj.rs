//! Command object implementation.
//!
//! A command object represents a command to be run on the command line.

use std::ffi::{OsStr, OsString};
use std::process::Command;

use super::traits::CredentialStrategy;

/// A command object representing a command to be run.
///
/// This is a simple data struct that holds the command configuration.
pub struct CmdObj {
    /// The underlying command.
    pub cmd: Command,
    /// Whether to skip logging.
    pub dont_log: bool,
    /// Whether to stream output.
    pub stream_output: bool,
    /// Whether to suppress output unless error.
    pub suppress_output_unless_error: bool,
    /// Whether to use PTY.
    pub use_pty: bool,
    /// Whether to ignore empty errors.
    pub ignore_empty_error: bool,
    /// Credential strategy.
    pub credential_strategy: CredentialStrategy,
}

impl CmdObj {
    /// Create a new command object.
    pub fn new(cmd: Command) -> Self {
        CmdObj {
            cmd,
            dont_log: false,
            stream_output: false,
            suppress_output_unless_error: false,
            use_pty: false,
            ignore_empty_error: false,
            credential_strategy: CredentialStrategy::None,
        }
    }

    /// Get the underlying command.
    pub fn get_cmd(&self) -> &Command {
        &self.cmd
    }

    /// Get the arguments as a string representation.
    pub fn to_string(&self) -> String {
        let args: Vec<_> = self.cmd.get_args().collect();
        if args.is_empty() {
            return String::new();
        }
        args.iter()
            .map(|arg| {
                let s = arg.to_string_lossy();
                if s.contains(' ') {
                    format!("\"{}\"", s)
                } else {
                    s.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Get the arguments.
    pub fn args(&self) -> Vec<&OsStr> {
        self.cmd.get_args().collect()
    }

    /// Add environment variables.
    pub fn add_env_vars(mut self, vars: &[&str]) -> Self {
        for var in vars {
            if let Some((key, value)) = var.split_once('=') {
                self.cmd.env(key, value);
            }
        }
        self
    }

    /// Set the working directory.
    pub fn set_wd(mut self, wd: &str) -> Self {
        self.cmd.current_dir(wd);
        self
    }

    /// Don't log this command.
    pub fn dont_log(mut self) -> Self {
        self.dont_log = true;
        self
    }

    /// Whether to log.
    pub fn should_log(&self) -> bool {
        !self.dont_log
    }

    /// Stream output to the command writer.
    pub fn stream_output(mut self) -> Self {
        self.stream_output = true;
        self
    }

    /// Suppress output unless there's an error.
    pub fn suppress_output_unless_error(mut self) -> Self {
        self.suppress_output_unless_error = true;
        self
    }

    /// Use a PTY for this command.
    pub fn use_pty(mut self) -> Self {
        self.use_pty = true;
        self
    }

    /// Ignore empty errors.
    pub fn ignore_empty_error(mut self) -> Self {
        self.ignore_empty_error = true;
        self
    }

    /// Prompt for credentials if requested.
    pub fn prompt_on_credential_request(mut self) -> Self {
        self.credential_strategy = CredentialStrategy::Prompt;
        self.use_pty = true;
        self
    }

    /// Fail on credential request.
    pub fn fail_on_credential_request(mut self) -> Self {
        self.credential_strategy = CredentialStrategy::Fail;
        self.use_pty = true;
        self
    }
}
