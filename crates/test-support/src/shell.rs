//! Shell operations for integration tests.

use std::ffi::OsStr;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

use super::env::{HOME, PATH, TERM};
use super::paths::Paths;

pub struct TestShell {
    pwd: PathBuf,
    env: Vec<(String, String)>,
    temp_dir: TempDir,
}

impl TestShell {
    pub fn new(paths: &Paths) -> io::Result<Self> {
        let temp_dir = TempDir::new()?;
        let actual_repo = paths.actual_repo();

        std::fs::create_dir_all(&actual_repo)?;

        let mut env_vars: Vec<(String, String)> = Vec::new();

        if let Ok(val) = std::env::var(PATH) {
            env_vars.push((PATH.to_string(), val));
        }
        if let Ok(val) = std::env::var(TERM) {
            env_vars.push((TERM.to_string(), val));
        }

        let home_path = temp_dir.path().to_string_lossy().to_string();
        env_vars.push((HOME.to_string(), home_path.clone()));

        let global_config_path = format!("{}/test/global_git_config", home_path);
        env_vars.push(("GIT_CONFIG_GLOBAL".to_string(), global_config_path));

        let mut shell = Self {
            pwd: actual_repo,
            env: env_vars,
            temp_dir,
        };

        shell.git_config_global()?;

        Ok(shell)
    }

    fn git_config_global(&self) -> io::Result<()> {
        let global_config = self.temp_dir.path().join("test/global_git_config");
        if let Some(parent) = global_config.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            &global_config,
            "[user]\n\tname = Test User\n\temail = test@example.com\n",
        )?;

        self.run(&[
            "git",
            "config",
            "--global",
            "include.path",
            global_config.to_str().unwrap_or(""),
        ])?;

        Ok(())
    }

    pub fn run<I, S>(&self, args: I) -> io::Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg(&self.shell_command(args))
            .envs(self.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .current_dir(&self.pwd);

        let output = cmd.output()?;
        Ok(output)
    }

    fn shell_command<I, S>(&self, args: I) -> String
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut cmd = String::from("cd ");
        cmd.push_str(&self.pwd.to_string_lossy());
        cmd.push_str(" && ");

        let args_str: Vec<String> = args
            .into_iter()
            .map(|s| s.as_ref().to_string_lossy().into_owned())
            .collect();

        cmd.push_str(&args_str.join(" "));
        cmd
    }

    pub fn run_with_input<I, S>(&self, args: I, input: &str) -> io::Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg(&self.shell_command(args))
            .envs(self.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .current_dir(&self.pwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());

        if let Ok(mut child) = cmd.spawn() {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(input.as_bytes())?;
            }
            child
                .wait_with_output()
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "Failed to spawn shell",
            ))
        }
    }

    pub fn pwd(&self) -> &Path {
        &self.pwd
    }

    pub fn set_pwd(&mut self, path: &Path) {
        self.pwd = path.to_path_buf();
    }

    pub fn delete(&self, path: &Path) -> io::Result<()> {
        if path.is_dir() {
            std::fs::remove_dir_all(path)
        } else {
            std::fs::remove_file(path)
        }
    }

    pub fn create_file(&self, path: &Path, contents: &str) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, contents)
    }

    pub fn append_file(&self, path: &Path, contents: &str) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let existing = if path.exists() {
            std::fs::read_to_string(path)?
        } else {
            String::new()
        };

        std::fs::write(path, existing + contents)
    }

    pub fn write_file(&self, relative_path: &str, contents: &str) -> io::Result<()> {
        let path = self.pwd.join(relative_path);
        self.create_file(&path, contents)
    }

    pub fn append(&self, relative_path: &str, contents: &str) -> io::Result<()> {
        let path = self.pwd.join(relative_path);
        self.append_file(&path, contents)
    }

    pub fn remove(&self, path: &Path) -> io::Result<()> {
        self.delete(path)
    }

    pub fn mkdir_all(&self, path: &Path) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }

    pub fn mkdir(&self, path: &Path) -> io::Result<()> {
        std::fs::create_dir(path)
    }

    pub fn touch(&self, path: &Path) -> io::Result<()> {
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(path)?;
        Ok(())
    }

    pub fn ls(&self, path: &Path) -> io::Result<Vec<String>> {
        let entries = std::fs::read_dir(path)?;
        let mut result = Vec::new();
        for entry in entries {
            if let Ok(entry) = entry {
                if let Some(name) = entry.path().file_name() {
                    result.push(name.to_string_lossy().into_owned());
                }
            }
        }
        Ok(result)
    }

    pub fn cat(&self, path: &Path) -> io::Result<String> {
        std::fs::read_to_string(path)
    }

    pub fn write_bytes(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, contents)
    }

    pub fn file_exists(&self, path: &Path) -> bool {
        path.exists()
    }

    pub fn copy(&self, from: &Path, to: &Path) -> io::Result<()> {
        if from.is_dir() {
            copy_dir(from, to)?;
        } else {
            std::fs::copy(from, to)?;
        }
        Ok(())
    }

    pub fn copy_to(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.copy(from, to)
    }

    pub fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }

    pub fn resolve_symlink(&self, path: &Path) -> io::Result<PathBuf> {
        std::fs::read_link(path)
    }

    pub fn symlink(&self, target: &Path, link: &Path) -> io::Result<()> {
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(target, link)?;
        }
        #[cfg(not(windows))]
        {
            std::os::unix::fs::symlink(target, link)?;
        }
        Ok(())
    }

    pub fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    pub fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    pub fn matches(&self, path: &Path, pattern: &str) -> io::Result<bool> {
        let content = self.cat(path)?;
        Ok(wildmatch(pattern, &content))
    }
}

fn copy_dir(src: &Path, dst: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dst)?;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

fn wildmatch(pattern: &str, text: &str) -> bool {
    let pattern = pattern.replace("**", "\x00\x01");
    let text = text.replace("**", "\x00\x01");

    let regex_pattern = regex::Regex::new(&format!(
        "^{}$",
        regex::escape(&pattern).replace("\x00\x01", ".*")
    ))
    .unwrap();

    regex_pattern.is_match(&text)
}

impl TestShell {
    pub fn git(&self, args: &[&str]) -> io::Result<()> {
        let output = self.run(args)?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "git command failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(())
    }

    pub fn git_with_input(&self, args: &[&str], input: &str) -> io::Result<()> {
        let output = self.run_with_input(args, input)?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "git command failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(())
    }

    pub fn shell(&self, args: &[&str]) -> io::Result<Output> {
        self.run(args)
    }

    pub fn env(&self) -> &[(String, String)] {
        &self.env
    }

    pub fn add_env(&mut self, key: &str, value: &str) {
        self.env.push((key.to_string(), value.to_string()));
    }

    pub fn set_env(&mut self, key: &str, value: &str) {
        self.add_env(key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_creation() -> io::Result<()> {
        let paths = Paths::new(PathBuf::from("/tmp/test_paths"));
        let shell = TestShell::new(&paths)?;

        assert!(shell.pwd().exists());
        Ok(())
    }
}
