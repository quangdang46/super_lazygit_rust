use std::process::Command;

use crate::{GitError, GitResult};

#[derive(Debug, Clone)]
pub struct CustomCommands {
    repo_path: std::path::PathBuf,
}

impl CustomCommands {
    #[must_use]
    pub fn new(repo_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
        }
    }

    pub fn run_with_output(&self, cmd_str: &str) -> GitResult<String> {
        let argv = parse_cmd_args(cmd_str)?;
        let (program, args) = argv.split_first().expect("custom command has argv");

        let output = Command::new(program)
            .args(args)
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| crate::GitError::OperationFailed {
                message: e.to_string(),
            })?;

        if !output.status.success() {
            return Err(crate::GitError::OperationFailed {
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn template_function_run_command(&self, cmd_str: &str) -> GitResult<String> {
        let output = self.run_with_output(cmd_str)?;
        let output = output.trim_end_matches(['\r', '\n']).to_string();

        if output.contains("\r\n") {
            return Err(GitError::OperationFailed {
                message: format!("command output contains newlines: {}", output),
            });
        }

        Ok(output)
    }
}

fn parse_cmd_args(command: &str) -> GitResult<Vec<String>> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum QuoteMode {
        Single,
        Double,
    }

    let mut args = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();
    let mut quote_mode = None;
    let mut token_started = false;

    while let Some(ch) = chars.next() {
        match quote_mode {
            Some(QuoteMode::Single) => {
                if ch == '\'' {
                    quote_mode = None;
                } else {
                    current.push(ch);
                }
                token_started = true;
            }
            Some(QuoteMode::Double) => {
                if ch == '"' {
                    quote_mode = None;
                } else if ch == '\\' {
                    let escaped = chars.next().ok_or_else(|| GitError::OperationFailed {
                        message: "unterminated escape in custom command".to_string(),
                    })?;
                    current.push(escaped);
                } else {
                    current.push(ch);
                }
                token_started = true;
            }
            None => match ch {
                '\'' => {
                    quote_mode = Some(QuoteMode::Single);
                    token_started = true;
                }
                '"' => {
                    quote_mode = Some(QuoteMode::Double);
                    token_started = true;
                }
                '\\' => {
                    let escaped = chars.next().ok_or_else(|| GitError::OperationFailed {
                        message: "unterminated escape in custom command".to_string(),
                    })?;
                    current.push(escaped);
                    token_started = true;
                }
                ch if ch.is_whitespace() => {
                    if token_started {
                        args.push(std::mem::take(&mut current));
                        token_started = false;
                    }
                }
                _ => {
                    current.push(ch);
                    token_started = true;
                }
            },
        }
    }

    if quote_mode.is_some() {
        return Err(GitError::OperationFailed {
            message: "unterminated quote in custom command".to_string(),
        });
    }

    if token_started {
        args.push(current);
    }

    if args.is_empty() {
        return Err(GitError::OperationFailed {
            message: "custom command is empty".to_string(),
        });
    }

    Ok(args)
}
