use std::io;

/// GUI IO structure for capturing command output and logging.
///
/// This struct captures IO operations for GUI display purposes:
/// - Logging commands being run
/// - Capturing command output
/// - Handling credential prompts
pub struct GuiIo {
    /// Whether to log commands
    pub log_commands: bool,
    /// Callback for logging a command
    pub log_command_fn: Box<dyn Fn(&str, bool) + Send + Sync>,
    /// Callback for creating a new command writer
    pub new_cmd_writer_fn: Box<dyn Fn() -> Box<dyn io::Write + Send> + Send + Sync>,
    /// Callback for prompting credentials
    pub prompt_for_credential_fn: Box<dyn Fn(CredentialType) -> String + Send + Sync>,
}

impl GuiIo {
    /// Create a new GuiIo with the given callbacks.
    #[must_use]
    pub fn new(
        log_commands: bool,
        log_command_fn: impl Fn(&str, bool) + Send + Sync + 'static,
        new_cmd_writer_fn: impl Fn() -> Box<dyn io::Write + Send> + Send + Sync + 'static,
        prompt_for_credential_fn: impl Fn(CredentialType) -> String + Send + Sync + 'static,
    ) -> Self {
        Self {
            log_commands,
            log_command_fn: Box::new(log_command_fn),
            new_cmd_writer_fn: Box::new(new_cmd_writer_fn),
            prompt_for_credential_fn: Box::new(prompt_for_credential_fn),
        }
    }

    /// Create a null GuiIo that discards all output and fails credential prompts.
    #[must_use]
    pub fn null() -> Self {
        Self {
            log_commands: false,
            log_command_fn: Box::new(|_, _| {}),
            new_cmd_writer_fn: Box::new(|| Box::new(io::Sink) as Box<dyn io::Write + Send>),
            prompt_for_credential_fn: Box::new(|_| String::new()),
        }
    }

    /// Log a command being executed.
    pub fn log_command(&self, cmd: &str, is_command_line_command: bool) {
        (self.log_command_fn)(cmd, is_command_line_command);
    }

    /// Get a new command writer.
    pub fn new_cmd_writer(&self) -> Box<dyn io::Write + Send> {
        (self.new_cmd_writer_fn)()
    }

    /// Prompt for a credential.
    pub fn prompt_for_credential(&self, credential: CredentialType) -> String {
        (self.prompt_for_credential_fn)(credential)
    }
}

impl Default for GuiIo {
    fn default() -> Self {
        Self::null()
    }
}

/// Types of credentials that can be requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialType {
    /// Username credential
    Username,
    /// Password credential
    Password,
    /// Passphrase credential
    Passphrase,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gui_io_null() {
        let gui_io = GuiIo::null();
        assert!(!gui_io.log_commands);
        gui_io.log_command("test", true);
        let _ = gui_io.new_cmd_writer();
        assert_eq!(gui_io.prompt_for_credential(CredentialType::Password), "");
    }

    #[test]
    fn test_gui_io_with_callbacks() {
        let gui_io = GuiIo::new(
            true,
            |cmd, is_cli| {
                assert_eq!(cmd, "git status");
                assert!(is_cli);
            },
            || Box::new(Vec::new()) as Box<dyn io::Write + Send>,
            |cred| match cred {
                CredentialType::Username => "user".to_string(),
                CredentialType::Password => "pass".to_string(),
                CredentialType::Passphrase => "phrase".to_string(),
            },
        );

        gui_io.log_command("git status", true);
        let writer = gui_io.new_cmd_writer();
        assert!(writer.write(b"test").is_ok());
        assert_eq!(
            gui_io.prompt_for_credential(CredentialType::Username),
            "user"
        );
    }
}
