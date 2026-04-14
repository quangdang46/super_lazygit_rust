mod cmd_obj;
mod cmd_obj_runner;
mod git_cmd_obj_builder;
mod git_cmd_obj_runner;
mod traits;

pub use cmd_obj::CmdObj;
pub use cmd_obj_runner::{CmdLogger, CmdObjRunner};
pub use git_cmd_obj_builder::{GitCmdObjBuilder, SharedGitCmdObjBuilder};
pub use git_cmd_obj_runner::{GitCmdLogger, GitCmdRunner, ThreadSafeGitCmdRunner};
pub use traits::{CmdRunner, RunError};

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
    platform: Platform,
    cmd: CmdObjBuilder,
    log: Arc<dyn CmdLogger>,
    runner: Arc<dyn CmdRunner>,
    temp_dir: String,
    open_command: String,
    open_link_command: String,
}

impl OSCommand {
    pub fn new(
        log: Arc<dyn CmdLogger>,
        temp_dir: &str,
        open_command: Option<String>,
        open_link_command: Option<String>,
    ) -> Self {
        let platform = Platform::default();
        let runner = Arc::new(CmdObjRunner::new(log.clone())) as Arc<dyn CmdRunner>;
        let cmd = CmdObjBuilder {
            runner: runner.clone(),
            platform: platform.clone(),
        };

        let open_cmd = open_command.unwrap_or_else(|| platform.open_command.clone());
        let open_link = open_link_command.unwrap_or_else(|| platform.open_link_command.clone());

        OSCommand {
            platform,
            cmd,
            log,
            runner,
            temp_dir: temp_dir.to_string(),
            open_command: open_cmd,
            open_link_command: open_link,
        }
    }

    pub fn new_shell(&self, command: &str) -> CmdObj {
        self.cmd
            .new_shell(command, None)
            .add_env_vars(&["GIT_OPTIONAL_LOCKS=0"])
    }

    pub fn new_shell_with_file(&self, command: &str, shell_functions_file: &str) -> CmdObj {
        self.cmd.new_shell(command, Some(shell_functions_file))
    }

    pub fn quote(&self, message: &str) -> String {
        self.cmd.quote(message)
    }

    pub fn get_temp_dir(&self) -> &str {
        &self.temp_dir
    }

    pub fn getenv(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }

    pub fn remove_file(&self, path: &Path) -> std::io::Result<()> {
        std::fs::remove_file(path)
    }

    pub fn file_exists(&self, path: &Path) -> bool {
        path.exists()
    }

    pub fn log_command(&self, cmd: &str, command_line: bool) {
        self.log.log_command(cmd, command_line);
    }

    pub fn run(&self, cmd_obj: CmdObj) -> Result<(), RunError> {
        self.runner.run(cmd_obj.cmd)
    }

    pub fn run_with_output(&self, cmd_obj: CmdObj) -> Result<String, RunError> {
        self.runner.run_with_output(cmd_obj.cmd)
    }

    pub fn run_with_outputs(&self, cmd_obj: CmdObj) -> Result<(String, String), RunError> {
        self.runner.run_with_outputs(cmd_obj.cmd)
    }

    /// Opens a file with the default application.
    pub fn open_file(&self, filename: &str) -> Result<(), RunError> {
        let template = &self.open_command;
        let command = template.replace("{filename}", &self.quote(filename));
        self.run(self.new_shell(&command))
    }

    /// Opens a link with the default application.
    pub fn open_link(&self, link: &str) -> Result<(), RunError> {
        let template = &self.open_link_command;
        let command = template.replace("{link}", &self.quote(link));
        self.run(self.new_shell(&command))
    }

    pub fn copy_to_clipboard(&self, text: &str) -> Result<(), RunError> {
        use std::io::Write;
        use std::process::{Command, Stdio};

        #[cfg(unix)]
        {
            if let Ok(mut child) = Command::new("xclip")
                .args(["-selection", "clipboard", "-i"])
                .stdin(Stdio::piped())
                .spawn()
            {
                if let Some(ref mut stdin) = child.stdin {
                    let _ = stdin.write_all(text.as_bytes());
                }
                let _ = child.wait();
                return Ok(());
            }
            if let Ok(mut child) = Command::new("xsel")
                .args(["--clipboard", "--input"])
                .stdin(Stdio::piped())
                .spawn()
            {
                if let Some(ref mut stdin) = child.stdin {
                    let _ = stdin.write_all(text.as_bytes());
                }
                let _ = child.wait();
                return Ok(());
            }
        }

        #[cfg(target_os = "macos")]
        {
            let mut child = Command::new("pbcopy")
                .stdin(Stdio::piped())
                .spawn()
                .map_err(|e| RunError::new(e.to_string()))?;
            if let Some(ref mut stdin) = child.stdin {
                stdin
                    .write_all(text.as_bytes())
                    .map_err(|e| RunError::new(e.to_string()))?;
            }
            child.wait().map_err(|e| RunError::new(e.to_string()))?;
            return Ok(());
        }

        Err(RunError::new("no clipboard tool available"))
    }

    pub fn paste_from_clipboard(&self) -> Result<String, RunError> {
        use std::process::Command;

        #[cfg(unix)]
        {
            if let Ok(output) = Command::new("xclip")
                .args(["-selection", "clipboard", "-o"])
                .output()
            {
                if output.status.success() {
                    return Ok(String::from_utf8_lossy(&output.stdout).to_string());
                }
            }
            if let Ok(output) = Command::new("xsel")
                .args(["--clipboard", "--output"])
                .output()
            {
                if output.status.success() {
                    return Ok(String::from_utf8_lossy(&output.stdout).to_string());
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            let output = Command::new("pbpaste")
                .output()
                .map_err(|e| RunError::new(e.to_string()))?;
            if output.status.success() {
                return Ok(String::from_utf8_lossy(&output.stdout).to_string());
            }
        }

        Err(RunError::new("no clipboard tool available"))
    }

    /// Appends a line to a file, creating it if necessary.
    pub fn append_line_to_file(&self, filename: &str, line: &str) -> Result<(), std::io::Error> {
        use std::io::Write;

        let content = std::fs::read_to_string(filename).unwrap_or_default();

        if !content.is_empty() && !content.ends_with('\n') {
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(filename)?;
            file.write_all(b"\n")?;
        }

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(filename)?;
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;

        Ok(())
    }

    /// Creates a file with the given content, including parent directories.
    pub fn create_file_with_content(
        &self,
        path: &Path,
        content: &str,
    ) -> Result<(), std::io::Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)
    }

    /// Removes a file or directory at the specified path.
    pub fn remove(&self, path: &Path) -> std::io::Result<()> {
        std::fs::remove_dir_all(path)
    }

    /// Determines the type of a file (file, directory, or other).
    pub fn file_type(path: &Path) -> &'static str {
        match std::fs::metadata(path) {
            Ok(meta) => {
                if meta.is_dir() {
                    "directory"
                } else {
                    "file"
                }
            }
            Err(_) => "other",
        }
    }

    pub fn pipe_commands(&self, cmd_objs: &[CmdObj]) -> Result<(), RunError> {
        if cmd_objs.is_empty() {
            return Ok(());
        }

        // Build a shell command that pipes all commands together
        let pipeline: Vec<String> = cmd_objs.iter().map(|obj| obj.to_string()).collect();
        let pipeline_cmd = pipeline.join(" | ");
        self.run(self.new_shell(&pipeline_cmd))
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
        let os_cmd = OSCommand::new(logger, "/tmp", None, None);
        let cmd = os_cmd.new_shell("echo hello");
        assert!(cmd.to_string().contains("echo"));
    }

    #[test]
    fn test_quote_unix() {
        let logger = Arc::new(MockCmdLogger::new());
        let os_cmd = OSCommand::new(logger, "/tmp", None, None);
        let quoted = os_cmd.quote("hello world");
        assert_eq!(quoted, "\"hello world\"");
    }
}
