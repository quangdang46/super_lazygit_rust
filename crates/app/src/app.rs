//! Main application setup and initialization.
//!
//! This module manages bootstrapping and running the application.

use std::io::{self, BufRead, Write};
use std::path::Path;
use anyhow::{Context, Result};

use super_lazygit_config::AppConfig;
use super_lazygit_core::{AppState, GitCommand, RepoSummary, RepoId};
use super_lazygit_git::GitFacade;
use super_lazygit_workspace::WorkspaceRegistry;
use crate::daemon::{self, DaemonKind};
use crate::runtime::AppRuntime;
use crate::updates::Updater;

/// Minimum required git version
const MIN_GIT_VERSION_STR: &str = "2.32.0";

/// App is the main application struct that manages bootstrapping and running the application.
pub struct App {
    /// Application state
    pub state: AppState,
    /// Application configuration
    pub config: AppConfig,
    /// Git command facade
    pub git: GitFacade,
    /// Application runtime
    pub runtime: Option<AppRuntime>,
    /// Workspace registry
    pub workspace: WorkspaceRegistry,
    /// Updater for checking updates
    pub updater: Option<Updater>,
}

impl App {
    /// Create a new application instance
    pub fn new(config: AppConfig, workspace: WorkspaceRegistry, git: GitFacade) -> Self {
        Self {
            state: AppState::default(),
            config,
            git,
            runtime: None,
            workspace,
            updater: None,
        }
    }

    /// Run the application
    pub fn run(&mut self) -> Result<()> {
        // Initialize runtime if not already done
        if self.runtime.is_none() {
            let mut app = crate::super_lazygit_tui::TuiApp::new(self.state.clone(), self.config.clone());
            self.runtime = Some(AppRuntime::new(app, self.workspace.clone(), self.git.clone()));
        }

        if let Some(runtime) = &mut self.runtime {
            runtime.run();
        }

        Ok(())
    }

    /// Close the application and clean up resources
    pub fn close(&mut self) -> Result<()> {
        // Clean up resources
        if let Some(runtime) = &mut self.runtime {
            // Runtime cleanup would go here
        }
        Ok(())
    }
}

/// Run the application with the given configuration
pub fn run(config: AppConfig, start_args: StartArgs) -> Result<()> {
    let mut app = App::new(config, WorkspaceRegistry::new(None), GitFacade::default());

    // Check git version
    let git_version = validate_git_version()?;
    if git_version.is_too_old() {
        anyhow::bail!(
            "Git version {} is too old. Minimum required version is {}",
            git_version.version(),
            MIN_GIT_VERSION_STR
        );
    }

    // TODO: Initialize updater, setup repo, create GUI, etc.

    app.run()
}

/// Start arguments for the application
#[derive(Debug, Clone)]
pub struct StartArgs {
    /// Filter path for git log
    pub filter_path: Option<String>,
    /// Git argument for initial panel focus
    pub git_arg: GitArg,
    /// Screen mode
    pub screen_mode: String,
    /// Integration test flag
    pub integration_test: bool,
}

impl StartArgs {
    /// Create new start arguments
    pub fn new(filter_path: Option<String>, git_arg: GitArg, screen_mode: String, integration_test: bool) -> Self {
        Self {
            filter_path,
            git_arg,
            screen_mode,
            integration_test,
        }
    }
}

/// Git argument for initial panel focus
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitArg {
    /// No argument specified
    None,
    /// Focus status panel
    Status,
    /// Focus branch panel
    Branch,
    /// Focus log panel
    Log,
    /// Focus stash panel
    Stash,
}

impl Default for GitArg {
    fn default() -> Self {
        GitArg::None
    }
}

/// Validate that git version meets minimum requirements
fn validate_git_version() -> Result<GitVersion> {
    let version = get_git_version()?;
    let min_version = parse_git_version(MIN_GIT_VERSION_STR);

    if version.is_older_than(&min_version) {
        anyhow::bail!(
            "Git version {} is too old. Minimum required version is {}",
            version.version(),
            MIN_GIT_VERSION_STR
        );
    }

    Ok(version)
}

/// Git version information
#[derive(Debug, Clone)]
pub struct GitVersion {
    version: String,
    major: u32,
    minor: u32,
    patch: u32,
}

impl GitVersion {
    /// Get the version string
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Check if this version is older than another version
    pub fn is_older_than(&self, other: &GitVersion) -> bool {
        if self.major != other.major {
            return self.major < other.major;
        }
        if self.minor != other.minor {
            return self.minor < other.minor;
        }
        self.patch < other.patch
    }

    /// Check if git version is too old for lazygit
    pub fn is_too_old(&self) -> bool {
        let min_version = parse_git_version(MIN_GIT_VERSION_STR);
        self.is_older_than(&min_version)
    }
}

/// Get the current git version
fn get_git_version() -> Result<GitVersion> {
    let output = std::process::Command::new("git")
        .args(["--version"])
        .output()
        .context("Failed to get git version")?;

    let output = String::from_utf8_lossy(&output.stdout);
    let version_str = output
        .trim()
        .strip_prefix("git version ")
        .unwrap_or(&output)
        .trim();

    parse_git_version(version_str)
}

/// Parse a git version string into components
fn parse_git_version(version_str: &str) -> GitVersion {
    let parts: Vec<&str> = version_str.split('.').collect();
    let (major, minor, patch) = if parts.len() >= 3 {
        (
            parts[0].parse().unwrap_or(0),
            parts[1].parse().unwrap_or(0),
            parts[2].parse().unwrap_or(0),
        )
    } else if parts.len() >= 2 {
        (
            parts[0].parse().unwrap_or(0),
            parts[1].parse().unwrap_or(0),
            0,
        )
    } else {
        (parts[0].parse().unwrap_or(0), 0, 0)
    };

    GitVersion {
        version: version_str.to_string(),
        major,
        minor,
        patch,
    }
}

/// Check if a directory is a git repository
pub fn is_git_repository(dir: &Path) -> bool {
    dir.join(".git").exists()
}

/// Setup repository based on current directory state
pub fn setup_repo() -> Result<(bool, Option<RepoId>)> {
    let cwd = std::env::current_dir()?;

    if !is_git_repository(&cwd) {
        // Not in a git repository
        match prompt_for_init() {
            true => {
                // Initialize repository
                init_repo()?;
                Ok((false, None))
            }
            false => {
                // Try to open recent repo
                if let Some(repo_id) = try_open_recent_repo() {
                    Ok((true, Some(repo_id)))
                } else {
                    anyhow::bail!("No recent repositories found");
                }
            }
        }
    } else {
        Ok((false, None))
    }
}

/// Prompt user for repository initialization
fn prompt_for_init() -> bool {
    print!("Create a new git repository? (y/n): ");
    io::stdout().flush().ok();

    let mut response = String::new();
    if io::stdin().lock().read_line(&mut response).is_ok() {
        response.trim().eq_ignore_ascii_case("y")
    } else {
        false
    }
}

/// Initialize a new git repository
fn init_repo() -> Result<()> {
    let mut cmd = std::process::Command::new("git");
    cmd.arg("init");

    // TODO: Handle initial branch option

    let status = cmd.status().context("Failed to initialize git repository")?;
    if !status.success() {
        anyhow::bail!("git init failed");
    }

    Ok(())
}

/// Try to open a recent repository from config
fn try_open_recent_repo() -> Option<RepoId> {
    // This would read from config's recent repos
    // For now, return None
    None
}

/// Check for daemon mode and handle if applicable
pub fn handle_daemon() -> bool {
    if daemon::in_daemon_mode() {
        let kind = daemon::get_daemon_kind();
        match kind {
            DaemonKind::ExitImmediately => {
                // Exit immediately without doing anything
                return true;
            }
            _ => {
                // TODO: Handle other daemon kinds
                anyhow::bail!("Daemon kind {:?} not yet implemented", kind);
            }
        }
    }
    false
}
