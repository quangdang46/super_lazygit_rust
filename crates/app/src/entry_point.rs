//! Entry point for the lazygit application.
//!
//! This module handles CLI argument parsing, environment setup,
//! and launching the main application.

use std::env;
use std::path::PathBuf;
use anyhow::{Context, Result};

use clap::{ArgAction, Parser, ValueEnum};

use super_lazygit_config::{default_config_toml, AppConfig, ConfigDiscovery};
use super_lazygit_core::{Action, AppState, Diagnostics, Event, RepoId, RepoSubview, ScreenMode};
use super_lazygit_git::GitFacade;
use super_lazygit_tui::TuiApp;
use super_lazygit_workspace::WorkspaceRegistry;

use crate::app::{self, App};
use crate::daemon;
use crate::runtime::AppRuntime;

/// Build information for the application
#[derive(Debug, Clone)]
pub struct BuildInfo {
    /// Git commit hash
    pub commit: String,
    /// Build date
    pub date: String,
    /// Version string
    pub version: String,
    /// Build source
    pub build_source: String,
}

impl Default for BuildInfo {
    fn default() -> Self {
        Self {
            commit: String::new(),
            date: String::new(),
            version: String::new(),
            build_source: String::new(),
        }
    }
}

/// CLI arguments for lazygit
#[derive(Debug, Parser, PartialEq, Eq)]
#[command(name = "lazygit", about = "A simple terminal UI for git commands")]
pub struct CliArgs {
    /// Path of git repo (equivalent to --work-tree=<path> --git-dir=<path>/.git/)
    #[arg(short = 'p', long = "path")]
    pub repo_path: Option<PathBuf>,

    /// Path to filter on in `git log -- <path>`
    #[arg(short = 'f', long = "filter")]
    pub filter_path: Option<PathBuf>,

    /// Panel to focus upon opening lazygit (status, branch, log, stash)
    #[arg(value_enum)]
    pub git_arg: Option<GitArg>,

    /// Print the current version
    #[arg(short = 'v', long = "version")]
    pub print_version_info: bool,

    /// Run in debug mode with logging
    #[arg(short = 'd', long = "debug")]
    pub debug: bool,

    /// Tail lazygit logs
    #[arg(short = 'l', long = "logs")]
    pub tail_logs: bool,

    /// Start the profiler on port 6060
    #[arg(long = "profile")]
    pub profile: bool,

    /// Print the default config
    #[arg(short = 'c', long = "config")]
    pub print_default_config: bool,

    /// Print the config directory
    #[arg(long = "print-config-dir")]
    pub print_config_dir: bool,

    /// Override default config directory
    #[arg(long = "use-config-dir", value_name = "DIR")]
    pub use_config_dir: Option<PathBuf>,

    /// Equivalent of the --work-tree git argument
    #[arg(short = 'w', long = "work-tree")]
    pub work_tree: Option<PathBuf>,

    /// Equivalent of the --git-dir git argument
    #[arg(short = 'g', long = "git-dir")]
    pub git_dir: Option<PathBuf>,

    /// Comma separated list of custom config file(s)
    #[arg(long = "use-config-file", value_name = "FILES")]
    pub custom_config_file: Option<String>,

    /// The initial screen-mode (normal, half, full)
    #[arg(long = "screen-mode", value_enum)]
    pub screen_mode: Option<ScreenModeArg>,

    /// Filter path argument (for internal use)
    #[arg(skip)]
    pub filter: Option<String>,
}

/// Screen mode argument
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ScreenModeArg {
    /// Normal screen mode
    Normal,
    /// Half screen mode
    Half,
    /// Full screen mode
    Full,
}

impl Default for ScreenModeArg {
    fn default() -> Self {
        ScreenModeArg::Normal
    }
}

/// Git argument for initial panel focus
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum GitArg {
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
        GitArg::Status
    }
}

/// Main entry point for the application
pub fn main() -> Result<()> {
    let cli_args = parse_cli_args();
    let build_info = get_build_info();

    // Handle CLI-specific actions that don't start the full app
    if cli_args.print_version_info {
        print_version_info(&build_info);
        return Ok(());
    }

    if cli_args.print_default_config {
        print_default_config()?;
        return Ok(());
    }

    if cli_args.print_config_dir {
        print_config_dir()?;
        return Ok(());
    }

    if cli_args.tail_logs {
        tail_logs()?;
        return Ok(());
    }

    // Handle repo path argument
    if let Some(repo_path) = &cli_args.repo_path {
        handle_repo_path(repo_path, &cli_args)?;
    }

    // Handle work tree
    if let Some(work_tree) = &cli_args.work_tree {
        env::set_var("GIT_WORK_TREE", work_tree);
        env::current_dir(work_tree)
            .with_context(|| format!("Failed to change directory to {}", work_tree.display()))?;
    }

    // Handle git dir
    if let Some(git_dir) = &cli_args.git_dir {
        env::set_var("GIT_DIR", git_dir);
    }

    // Handle custom config file
    if let Some(config_file) = &cli_args.custom_config_file {
        env::set_var("LG_CONFIG_FILE", config_file);
    }

    // Handle config directory override
    if let Some(config_dir) = &cli_args.use_config_dir {
        env::set_var("CONFIG_DIR", config_dir);
    }

    // Check for daemon mode
    if daemon::in_daemon_mode() {
        handle_daemon_mode()?;
        return Ok(());
    }

    // Create temp directory for this session
    let temp_dir = create_temp_dir()?;

    // Build configuration
    let config = build_config(&build_info, &cli_args, &temp_dir)?;

    // Start the application
    let start_args = app::StartArgs::new(
        cli_args.filter_path.map(|p| p.display().to_string()),
        cli_args.git_arg.map(|g| match g {
            GitArg::Status => app::GitArg::Status,
            GitArg::Branch => app::GitArg::Branch,
            GitArg::Log => app::GitArg::Log,
            GitArg::Stash => app::GitArg::Stash,
        }).unwrap_or(app::GitArg::None),
        cli_args.screen_mode.map(|s| match s {
            ScreenModeArg::Normal => "normal".to_string(),
            ScreenModeArg::Half => "half".to_string(),
            ScreenModeArg::Full => "full".to_string(),
        }).unwrap_or_default(),
        false, // integration_test
    );

    app::run(config, start_args)
}

/// Parse command line arguments
fn parse_cli_args() -> CliArgs {
    CliArgs::parse()
}

/// Get build information
fn get_build_info() -> BuildInfo {
    let mut info = BuildInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        ..Default::default()
    };

    // Try to read build info from environment or defaults
    if info.commit.is_empty() {
        info.commit = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
    }

    info
}

/// Print version information
fn print_version_info(build_info: &BuildInfo) {
    let git_version = get_git_version();
    println!(
        "commit={}, build date={}, build source={}, version={}, os={}, arch={}, git version={}",
        build_info.commit,
        build_info.date,
        build_info.build_source,
        build_info.version,
        std::env::consts::OS,
        std::env::consts::ARCH,
        git_version,
    );
}

/// Get git version string
fn get_git_version() -> String {
    std::process::Command::new("git")
        .args(["--version"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

/// Print default configuration
fn print_default_config() -> Result<()> {
    println!("{}", default_config_toml()?);
    Ok(())
}

/// Print config directory
fn print_config_dir() -> Result<()> {
    let config_discovery = ConfigDiscovery::from_overrides(None, None);
    let config_dir = config_discovery.config_dir()
        .context("could not determine config directory")?;
    println!("{}", config_dir.display());
    Ok(())
}

/// Tail logs (placeholder - would need logs::tail implementation)
fn tail_logs() -> Result<()> {
    // TODO: Implement log tailing
    anyhow::bail!("Log tailing not yet implemented")
}

/// Handle repo path argument
fn handle_repo_path(repo_path: &PathBuf, cli_args: &CliArgs) -> Result<()> {
    if cli_args.work_tree.is_some() || cli_args.git_dir.is_some() {
        anyhow::bail!("--path option is incompatible with --work-tree and --git-dir options");
    }

    let abs_repo_path = repo_path.canonicalize()
        .with_context(|| format!("Failed to resolve path: {}", repo_path.display()))?;

    if !is_git_repository(&abs_repo_path) {
        anyhow::bail!("{} is not a valid git repository", abs_repo_path.display());
    }

    let git_dir = abs_repo_path.join(".git");
    env::set_var("GIT_DIR", &git_dir);
    env::current_dir(&abs_repo_path)
        .with_context(|| format!("Failed to change directory to {}", abs_repo_path.display()))?;

    Ok(())
}

/// Check if directory is a git repository
fn is_git_repository(dir: &PathBuf) -> bool {
    dir.join(".git").exists()
}

/// Create temporary directory for the session
fn create_temp_dir() -> Result<PathBuf> {
    let temp_base = std::env::temp_dir();
    let temp_dir = temp_base.join("lazygit");

    std::fs::create_dir_all(&temp_dir)
        .with_context(|| format!("Failed to create temp directory: {}", temp_dir.display()))?;

    Ok(temp_dir)
}

/// Build application configuration
fn build_config(build_info: &BuildInfo, cli_args: &CliArgs, temp_dir: &PathBuf) -> Result<AppConfig> {
    // Create a default config for now
    // TODO: Implement proper config loading with user config
    Ok(AppConfig::default())
}

/// Handle daemon mode
fn handle_daemon_mode() -> Result<()> {
    let kind = daemon::get_daemon_kind();

    match kind {
        daemon::DaemonKind::ExitImmediately => {
            // Exit immediately
            Ok(())
        }
        daemon::DaemonKind::RemoveUpdateRefsForCopiedBranch => {
            // TODO: Implement
            anyhow::bail!("RemoveUpdateRefsForCopiedBranch not yet implemented")
        }
        daemon::DaemonKind::ChangeTodoActions => {
            // TODO: Implement
            anyhow::bail!("ChangeTodoActions not yet implemented")
        }
        daemon::DaemonKind::DropMergeCommit => {
            // TODO: Implement
            anyhow::bail!("DropMergeCommit not yet implemented")
        }
        daemon::DaemonKind::MoveFixupCommitDown => {
            // TODO: Implement
            anyhow::bail!("MoveFixupCommitDown not yet implemented")
        }
        daemon::DaemonKind::MoveTodosUp => {
            // TODO: Implement
            anyhow::bail!("MoveTodosUp not yet implemented")
        }
        daemon::DaemonKind::MoveTodosDown => {
            // TODO: Implement
            anyhow::bail!("MoveTodosDown not yet implemented")
        }
        daemon::DaemonKind::InsertBreak => {
            // TODO: Implement
            anyhow::bail!("InsertBreak not yet implemented")
        }
        daemon::DaemonKind::WriteRebaseTodo => {
            // TODO: Implement
            anyhow::bail!("WriteRebaseTodo not yet implemented")
        }
        daemon::DaemonKind::Unknown => {
            anyhow::bail!("Unknown daemon kind")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_arg_default() {
        assert_eq!(GitArg::default(), GitArg::Status);
    }

    #[test]
    fn test_screen_mode_arg_default() {
        assert_eq!(ScreenModeArg::default(), ScreenModeArg::Normal);
    }

    #[test]
    fn test_build_info_default() {
        let info = BuildInfo::default();
        assert!(info.commit.is_empty());
        assert!(info.date.is_empty());
        assert!(info.version.is_empty());
    }
}
