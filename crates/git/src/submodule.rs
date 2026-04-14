use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;

use regex::Regex;
use thiserror::Error;

use crate::{GitCommandBuilder, GitResult};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubmoduleConfig {
    pub name: String,
    pub path: String,
    pub url: String,
    pub parent_module: Option<Box<SubmoduleConfig>>,
}

impl SubmoduleConfig {
    pub fn full_path(&self) -> PathBuf {
        if let Some(ref parent) = self.parent_module {
            parent.full_path().join(&self.path)
        } else {
            PathBuf::from(&self.path)
        }
    }

    pub fn git_dir_path(&self, repo_git_dir_path: &Path) -> PathBuf {
        self.name
            .split('/')
            .filter(|segment| !segment.is_empty())
            .fold(repo_git_dir_path.to_path_buf(), |path, segment| {
                path.join("modules").join(segment)
            })
    }
}

#[derive(Debug, Error)]
pub enum SubmoduleError {
    #[error("git command failed: {0}")]
    GitError(String),
    #[error("submodule path does not exist: {0}")]
    PathNotExist(String),
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("regex error: {0}")]
    RegexError(#[from] regex::Error),
}

pub struct SubmoduleCommands {
    repo_path: PathBuf,
    repo_git_dir_path: PathBuf,
}

impl SubmoduleCommands {
    pub fn new(repo_path: impl Into<PathBuf>, repo_git_dir_path: impl Into<PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
            repo_git_dir_path: repo_git_dir_path.into(),
        }
    }

    fn run_git_cmd(&self, cmd: GitCommandBuilder) -> GitResult<()> {
        let argv = cmd.to_argv();
        let output = Command::new(&argv[0])
            .args(&argv[1..])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| crate::GitError::OperationFailed {
                message: format!("failed to run git command: {}", e),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::GitError::OperationFailed {
                message: format!("git command failed: {}", stderr),
            });
        }
        Ok(())
    }

    pub fn get_configs(
        &self,
        parent_module: Option<&SubmoduleConfig>,
    ) -> GitResult<Vec<SubmoduleConfig>> {
        let git_modules_path = if let Some(parent) = parent_module {
            parent.full_path().join(".gitmodules")
        } else {
            PathBuf::from(".gitmodules")
        };

        let file = match fs::File::open(&git_modules_path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Vec::new());
            }
            Err(e) => {
                return Err(crate::GitError::OperationFailed {
                    message: format!("failed to open .gitmodules: {}", e),
                })
            }
        };

        let reader = BufReader::new(file);
        let mut configs: Vec<SubmoduleConfig> = Vec::new();
        let mut last_config_idx: isize = -1;

        let first_match = |s: &str, regex: &str| -> Option<String> {
            let re = Regex::new(regex).ok()?;
            let matches = re.find(s)?;
            matches
                .as_str()
                .strip_prefix('[')
                .and_then(|s| s.strip_prefix("submodule \""))
                .and_then(|s| s.strip_suffix("\"]"))
                .map(|name| name.to_string())
        };

        let line_matches = |line: &str, regex: &str| -> Option<String> {
            let re = Regex::new(regex).ok()?;
            let caps = re.captures(line)?;
            caps.get(1).map(|m| m.as_str().to_string())
        };

        for line in reader.split(b'\n').filter_map(|l| l.ok()) {
            let line = String::from_utf8_lossy(&line).to_string();

            if let Some(name) = first_match(&line, r#"\[\s*submodule\s+"([^"]+)"\]"#) {
                let parent = parent_module.map(|p| {
                    let config = p.clone();
                    Box::new(config)
                });

                configs.push(SubmoduleConfig {
                    name,
                    path: String::new(),
                    url: String::new(),
                    parent_module: parent,
                });
                last_config_idx = configs.len() as isize - 1;
                continue;
            }

            if last_config_idx != -1 {
                let idx = last_config_idx as usize;

                if let Some(path) = line_matches(&line, r#"^\s*path\s*=\s*(.+?)\s*$"#) {
                    configs[idx].path = path;

                    let nested_configs = self.get_configs(Some(&configs[idx]))?;
                    if !nested_configs.is_empty() {
                        configs.extend(nested_configs);
                    }
                } else if let Some(url) = line_matches(&line, r#"^\s*url\s*=\s*(.+?)\s*$"#) {
                    configs[idx].url = url;
                }
            }
        }

        Ok(configs)
    }

    pub fn stash(&self, submodule: &SubmoduleConfig) -> GitResult<()> {
        if !submodule.full_path().exists() {
            return Ok(());
        }

        let cmd = GitCommandBuilder::new("stash")
            .dir(submodule.full_path())
            .arg(["--include-untracked"])
            .to_argv();

        let output = Command::new(&cmd[0])
            .args(&cmd[1..])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| crate::GitError::OperationFailed {
                message: format!("failed to run git stash: {}", e),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::GitError::OperationFailed {
                message: format!("git stash failed: {}", stderr),
            });
        }

        Ok(())
    }

    pub fn reset(&self, submodule: &SubmoduleConfig) -> GitResult<()> {
        let parent_dir = if let Some(ref parent) = submodule.parent_module {
            parent.full_path().to_string_lossy().to_string()
        } else {
            String::new()
        };

        let mut cmd = GitCommandBuilder::new("submodule").arg([
            "update",
            "--init",
            "--force",
            "--",
            &submodule.path,
        ]);

        if !parent_dir.is_empty() {
            cmd = cmd.dir(&parent_dir);
        }

        self.run_git_cmd(cmd)
    }

    pub fn update_all(&self) -> GitResult<()> {
        self.run_git_cmd(GitCommandBuilder::new("submodule").arg(["update", "--force"]))
    }

    pub fn delete(&self, submodule: &SubmoduleConfig) -> GitResult<()> {
        let original_cwd =
            std::env::current_dir().map_err(|e| crate::GitError::OperationFailed {
                message: format!("failed to get current directory: {}", e),
            })?;

        if let Some(ref parent) = submodule.parent_module {
            std::env::set_current_dir(parent.full_path()).map_err(|e| {
                crate::GitError::OperationFailed {
                    message: format!("failed to change directory: {}", e),
                }
            })?;
        }

        let deinit_result = self.run_git_cmd(GitCommandBuilder::new("submodule").arg([
            "deinit",
            "--force",
            "--",
            &submodule.path,
        ]));

        if let Err(e) = deinit_result {
            if !e
                .to_string()
                .contains("did not match any file(s) known to git")
            {
                let _ = std::env::set_current_dir(&original_cwd);
                return Err(e);
            }

            let config_remove_gitmodules =
                self.run_git_cmd(GitCommandBuilder::new("config").arg([
                    "--file",
                    ".gitmodules",
                    "--remove-section",
                    &format!("submodule.{}", submodule.path),
                ]));

            if let Err(e) = config_remove_gitmodules {
                let _ = std::env::set_current_dir(&original_cwd);
                return Err(e);
            }

            let config_remove = self.run_git_cmd(
                GitCommandBuilder::new("config")
                    .arg(["--remove-section", &format!("submodule.{}", submodule.path)]),
            );

            if let Err(e) = config_remove {
                let _ = std::env::set_current_dir(&original_cwd);
                return Err(e);
            }
        }

        let rm_result =
            self.run_git_cmd(GitCommandBuilder::new("rm").arg(["--force", "-r", &submodule.path]));

        if let Err(_e) = rm_result {}

        let _ = std::env::set_current_dir(&original_cwd);

        let git_dir = submodule.git_dir_path(&self.repo_git_dir_path);
        if git_dir.exists() {
            fs::remove_dir_all(&git_dir).map_err(|e| crate::GitError::OperationFailed {
                message: format!("failed to remove git dir: {}", e),
            })?;
        }

        Ok(())
    }

    pub fn add(&self, name: &str, path: &str, url: &str) -> GitResult<()> {
        self.run_git_cmd(
            GitCommandBuilder::new("submodule")
                .arg(["add", "--force", "--name", name, "--", url, path]),
        )
    }

    pub fn update_url(&self, submodule: &SubmoduleConfig, new_url: &str) -> GitResult<()> {
        let original_cwd =
            std::env::current_dir().map_err(|e| crate::GitError::OperationFailed {
                message: format!("failed to get current directory: {}", e),
            })?;

        if let Some(ref parent) = submodule.parent_module {
            std::env::set_current_dir(parent.full_path()).map_err(|e| {
                crate::GitError::OperationFailed {
                    message: format!("failed to change directory: {}", e),
                }
            })?;
        }

        let set_url_cmd = GitCommandBuilder::new("config").arg([
            "--file",
            ".gitmodules",
            &format!("submodule.{}.url", submodule.name),
            new_url,
        ]);

        if let Err(e) = self.run_git_cmd(set_url_cmd) {
            let _ = std::env::set_current_dir(&original_cwd);
            return Err(e);
        }

        let sync_cmd = GitCommandBuilder::new("submodule").arg(["sync", "--", &submodule.path]);

        let result = self.run_git_cmd(sync_cmd);

        let _ = std::env::set_current_dir(&original_cwd);

        result
    }

    pub fn init(&self, path: &str) -> GitResult<()> {
        self.run_git_cmd(GitCommandBuilder::new("submodule").arg(["init", "--", path]))
    }

    pub fn update(&self, path: &str) -> GitResult<()> {
        self.run_git_cmd(GitCommandBuilder::new("submodule").arg(["update", "--init", "--", path]))
    }

    pub fn bulk_init_cmd(&self) -> GitCommandBuilder {
        GitCommandBuilder::new("submodule").arg(["init"])
    }

    pub fn bulk_update_cmd(&self) -> GitCommandBuilder {
        GitCommandBuilder::new("submodule").arg(["update"])
    }

    pub fn force_bulk_update_cmd(&self) -> GitCommandBuilder {
        GitCommandBuilder::new("submodule").arg(["update", "--force"])
    }

    pub fn bulk_update_recursively_cmd(&self) -> GitCommandBuilder {
        GitCommandBuilder::new("submodule").arg(["update", "--init", "--recursive"])
    }

    pub fn bulk_deinit_cmd(&self) -> GitCommandBuilder {
        GitCommandBuilder::new("submodule").arg(["deinit", "--all", "--force"])
    }

    pub fn reset_submodules(&self, submodules: &[SubmoduleConfig]) -> GitResult<()> {
        for submodule in submodules {
            self.stash(submodule)?;
        }
        self.update_all()
    }
}
