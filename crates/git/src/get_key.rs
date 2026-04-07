use std::ffi::OsStr;
use std::process::{Command, Output};

/// Run a git config command and return the output.
pub fn run_git_config_cmd(cmd: &mut Command) -> Result<String, std::io::Error> {
    let output = cmd.output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout)
            .trim_end_matches('\0')
            .to_string())
    } else {
        // Check if it's a "key not found" error (exit code 1)
        // git config returns exit code 1 when the key is not found
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "the key is not found for {:?}",
                cmd.get_args().collect::<Vec<_>>()
            ),
        ))
    }
}

/// Get a git config command for retrieving a key.
pub fn get_git_config_cmd(key: &str) -> Command {
    let mut cmd = Command::new("git");
    cmd.args(["config", "--get", "--null", key]);
    cmd
}

/// Get a git config command with arbitrary arguments.
pub fn get_git_config_general_cmd(args: &str) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("config");
    cmd.args(args.split_whitespace());
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_git_config_cmd() {
        let mut cmd = get_git_config_cmd("user.name");
        assert_eq!(cmd.get_program(), "git");
    }

    #[test]
    fn test_get_git_config_general_cmd() {
        let cmd = get_git_config_general_cmd("--local --get-regexp user.*");
        assert_eq!(cmd.get_program(), "git");
    }
}
