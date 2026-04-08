use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use super::traits::{CmdRunner, RunError};

const INDEX_LOCK_MARKER: &str = ".git/index.lock";
const RETRY_COUNT: usize = 5;
const RETRY_WAIT: Duration = Duration::from_millis(50);

pub trait GitCmdLogger: Send + Sync {
    fn log_command(&self, cmd: &str, command_line: bool);
    fn warn(&self, msg: &str);
}

pub struct GitCmdRunner<R: CmdRunner> {
    inner: R,
    logger: Option<Arc<dyn GitCmdLogger>>,
}

impl<R: CmdRunner> GitCmdRunner<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            logger: None,
        }
    }

    pub fn with_logger(inner: R, logger: Arc<dyn GitCmdLogger>) -> Self {
        Self {
            inner,
            logger: Some(logger),
        }
    }

    fn is_index_lock_error(output: &str, stderr: &str) -> bool {
        output.contains(INDEX_LOCK_MARKER) || stderr.contains(INDEX_LOCK_MARKER)
    }

    fn log_warning(&self, msg: &str) {
        if let Some(ref logger) = self.logger {
            logger.warn(msg);
        }
    }

    pub fn run<F>(&self, cmd_factory: F) -> Result<(), RunError>
    where
        F: Fn() -> Command,
    {
        for _ in 0..RETRY_COUNT {
            match self.inner.run(cmd_factory()) {
                Ok(()) => return Ok(()),
                Err(e) => return Err(e),
            }
        }
        Err(RunError::new("command failed after retries"))
    }

    pub fn run_with_output<F>(&self, cmd_factory: F) -> Result<String, RunError>
    where
        F: Fn() -> Command,
    {
        for _ in 0..RETRY_COUNT {
            let output = self.inner.run_with_output(cmd_factory());
            match output {
                Ok(out) => {
                    let stderr = String::new();
                    if Self::is_index_lock_error(&out, &stderr) {
                        self.log_warning("index.lock prevented command from running. Retrying command after a small wait");
                        std::thread::sleep(RETRY_WAIT);
                        continue;
                    }
                    return Ok(out);
                }
                Err(e) => {
                    if Self::is_index_lock_error(&e.message, &e.message) {
                        self.log_warning("index.lock prevented command from running. Retrying command after a small wait");
                        std::thread::sleep(RETRY_WAIT);
                        continue;
                    }
                    return Err(e);
                }
            }
        }
        Err(RunError::new("command failed after retries"))
    }

    pub fn run_with_outputs<F>(&self, cmd_factory: F) -> Result<(String, String), RunError>
    where
        F: Fn() -> Command,
    {
        for _ in 0..RETRY_COUNT {
            let result = self.inner.run_with_outputs(cmd_factory());
            match result {
                Ok((stdout, stderr)) => {
                    if Self::is_index_lock_error(&stdout, &stderr) {
                        self.log_warning("index.lock prevented command from running. Retrying command after a small wait");
                        std::thread::sleep(RETRY_WAIT);
                        continue;
                    }
                    return Ok((stdout, stderr));
                }
                Err(e) => {
                    if Self::is_index_lock_error(&e.message, &e.message) {
                        self.log_warning("index.lock prevented command from running. Retrying command after a small wait");
                        std::thread::sleep(RETRY_WAIT);
                        continue;
                    }
                    return Err(e);
                }
            }
        }
        Err(RunError::new("command failed after retries"))
    }
}

pub struct ThreadSafeGitCmdRunner(Arc<dyn CmdRunner>);

impl ThreadSafeGitCmdRunner {
    pub fn new(runner: impl CmdRunner + 'static) -> Self {
        ThreadSafeGitCmdRunner(Arc::new(runner))
    }
}

impl CmdRunner for ThreadSafeGitCmdRunner {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct MockGitCmdLogger {
        warnings: Mutex<Vec<String>>,
    }

    impl MockGitCmdLogger {
        fn new() -> Self {
            MockGitCmdLogger {
                warnings: Mutex::new(Vec::new()),
            }
        }
    }

    impl GitCmdLogger for MockGitCmdLogger {
        fn log_command(&self, _cmd: &str, _command_line: bool) {}
        fn warn(&self, msg: &str) {
            self.warnings.lock().unwrap().push(msg.to_string());
        }
    }

    #[test]
    fn test_index_lock_detection() {
        assert!(
            GitCmdRunner::<crate::oscommands::CmdObjRunner>::is_index_lock_error(
                "error: cannot lock .git/index.lock",
                ""
            )
        );

        assert!(
            !GitCmdRunner::<crate::oscommands::CmdObjRunner>::is_index_lock_error("一切正常", "")
        );
    }
}
