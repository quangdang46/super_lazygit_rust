use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};

mod runtime;
mod terminal;
mod watcher;

use runtime::AppRuntime;
use super_lazygit_config::{default_config_toml, AppConfig, ConfigDiscovery};
use super_lazygit_core::{Action, AppState, Diagnostics, Event, RepoId, RepoSubview, ScreenMode};
use super_lazygit_git::GitFacade;
use super_lazygit_tui::TuiApp;
use super_lazygit_workspace::WorkspaceRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum StartupScreenMode {
    Normal,
    Half,
    Fullscreen,
}

impl From<StartupScreenMode> for ScreenMode {
    fn from(value: StartupScreenMode) -> Self {
        match value {
            StartupScreenMode::Normal => Self::Normal,
            StartupScreenMode::Half => Self::HalfScreen,
            StartupScreenMode::Fullscreen => Self::FullScreen,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum StartupFocus {
    Status,
    Branch,
    Log,
    Stash,
}

impl StartupFocus {
    const fn repo_subview(self) -> Option<RepoSubview> {
        match self {
            Self::Status => None,
            Self::Branch => Some(RepoSubview::Branches),
            Self::Log => Some(RepoSubview::Commits),
            Self::Stash => Some(RepoSubview::Stash),
        }
    }
}

#[derive(Debug, Parser, PartialEq, Eq)]
#[command(name = "super-lazygit", version)]
#[command(about = "Workspace-first Lazygit-grade Rust TUI")]
struct Cli {
    /// Path to the workspace root to open.
    #[arg(long)]
    workspace: Option<PathBuf>,
    /// Print the default config and exit.
    #[arg(long = "config")]
    print_config: bool,
    /// Print the effective config directory and exit.
    #[arg(long)]
    print_config_dir: bool,
    /// Prefer a specific config directory when resolving config.
    #[arg(long, value_name = "DIR")]
    use_config_dir: Option<PathBuf>,
    /// Load config from a specific file.
    #[arg(long, value_name = "FILE")]
    use_config_file: Option<PathBuf>,
    /// Override the initial screen mode.
    #[arg(long, value_enum)]
    screen_mode: Option<StartupScreenMode>,
    /// Focus a repository subview immediately after startup.
    #[arg(value_enum)]
    focus: Option<StartupFocus>,
}

fn main() -> Result<()> {
    let startup_started_at = Instant::now();
    let cli = Cli::parse();
    let config_discovery = resolve_config_discovery(&cli);

    if cli.print_config {
        print!("{}", default_config_toml()?);
        return Ok(());
    }

    if cli.print_config_dir {
        println!(
            "{}",
            config_discovery
                .config_dir()
                .context("could not determine a config directory from CLI or environment")?
                .display()
        );
        return Ok(());
    }

    let config_path_hint = config_discovery
        .primary_config_path()
        .map(std::path::Path::to_path_buf);
    let loaded_config = AppConfig::load_with_discovery(config_discovery)?;
    let config = loaded_config.config;
    let workspace_root = resolve_workspace_root(cli.workspace, &config);
    let mut state = AppState {
        config_path: loaded_config
            .source
            .path()
            .map(std::path::Path::to_path_buf),
        repository_url: option_env!("CARGO_PKG_REPOSITORY").map(str::to_string),
        ..AppState::default()
    };
    if state.config_path.is_none() {
        state.config_path = config_path_hint;
    }
    if let Some(screen_mode) = cli.screen_mode {
        state.settings.screen_mode = screen_mode.into();
    }
    let app = TuiApp::new(state, config.clone());
    let workspace = WorkspaceRegistry::new(workspace_root);
    let git = GitFacade::default();

    let mut diagnostics = Diagnostics::default();

    let mut runtime = AppRuntime::new(app, workspace, git);
    diagnostics.extend_snapshot(runtime.bootstrap()?);
    runtime.run([Event::Action(Action::RefreshVisibleRepos)]);
    apply_startup_focus(&mut runtime, cli.focus);

    let interactive_terminal = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    if interactive_terminal {
        terminal::run(&mut runtime)?;
    }

    diagnostics.extend_snapshot(runtime.diagnostics_snapshot());
    diagnostics.record_startup_stage("app.main", startup_started_at.elapsed());

    if config.diagnostics.enabled && config.diagnostics.log_samples {
        let snapshot = diagnostics.snapshot();
        eprintln!(
            "[diagnostics] app_main startup_total_ms={} stages={}",
            snapshot.startup_total.as_millis(),
            snapshot.startup.len()
        );
    }

    Ok(())
}

fn resolve_config_discovery(cli: &Cli) -> ConfigDiscovery {
    ConfigDiscovery::from_overrides(cli.use_config_dir.clone(), cli.use_config_file.clone())
}

fn startup_focus_events(repo_id: RepoId, focus: StartupFocus) -> Vec<Event> {
    let mut events = vec![Event::Action(Action::EnterRepoMode { repo_id })];
    if let Some(subview) = focus.repo_subview() {
        events.push(Event::Action(Action::SwitchRepoSubview(subview)));
    }
    events
}

fn apply_startup_focus(runtime: &mut AppRuntime, focus: Option<StartupFocus>) {
    let Some(focus) = focus else {
        return;
    };
    let Some(repo_id) = runtime.app().state().workspace.selected_repo_id.clone() else {
        return;
    };

    runtime.run(startup_focus_events(repo_id, focus));
}

fn resolve_workspace_root(cli_workspace: Option<PathBuf>, config: &AppConfig) -> Option<PathBuf> {
    cli_workspace
        .or_else(|| config.workspace.roots.first().cloned())
        .or_else(|| std::env::current_dir().ok())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{Duration, Instant};

    use super::watcher::{ScriptedWatcherBackend, ScriptedWatcherHandle};
    use super::*;
    use super_lazygit_core::{
        AppMode, AppWatcherEvent, BackgroundJobKind, BackgroundJobState, CommitHistoryMode, Event,
        PaneId, RepoId, RepoSubview, RepoSummary, ScanStatus, ScreenMode, TimerEvent, Timestamp,
        WatcherEventKind, WatcherHealth, WorkerEvent, WorkspaceState,
    };
    use super_lazygit_test_support::{clean_repo, TempRepo};

    fn normalized_path(path: &Path) -> PathBuf {
        path.canonicalize()
            .unwrap_or_else(|_| path.components().collect::<PathBuf>())
    }

    fn normalized_repo_id(path: &Path) -> RepoId {
        RepoId::new(normalized_path(path).display().to_string())
    }

    #[test]
    fn cli_parses_startup_flags_and_focus_argument() {
        let cli = Cli::try_parse_from([
            "super-lazygit",
            "--workspace",
            "/tmp/workspace",
            "--use-config-dir",
            "/tmp/config",
            "--screen-mode",
            "half",
            "stash",
        ])
        .expect("cli parse");

        assert_eq!(
            cli,
            Cli {
                workspace: Some(PathBuf::from("/tmp/workspace")),
                print_config: false,
                print_config_dir: false,
                use_config_dir: Some(PathBuf::from("/tmp/config")),
                use_config_file: None,
                screen_mode: Some(StartupScreenMode::Half),
                focus: Some(StartupFocus::Stash),
            }
        );
    }

    #[test]
    fn startup_screen_mode_maps_to_core_screen_mode() {
        assert_eq!(
            ScreenMode::from(StartupScreenMode::Normal),
            ScreenMode::Normal
        );
        assert_eq!(
            ScreenMode::from(StartupScreenMode::Half),
            ScreenMode::HalfScreen
        );
        assert_eq!(
            ScreenMode::from(StartupScreenMode::Fullscreen),
            ScreenMode::FullScreen
        );
    }

    #[test]
    fn config_discovery_prefers_cli_file_over_cli_dir() {
        let cli = Cli {
            workspace: None,
            print_config: false,
            print_config_dir: false,
            use_config_dir: Some(PathBuf::from("/tmp/config-dir")),
            use_config_file: Some(PathBuf::from("/tmp/config-file.toml")),
            screen_mode: None,
            focus: None,
        };

        let discovery = resolve_config_discovery(&cli);

        assert_eq!(
            discovery.primary_config_path(),
            Some(Path::new("/tmp/config-file.toml"))
        );
        assert_eq!(discovery.config_dir(), Some(Path::new("/tmp")));
    }

    #[test]
    fn startup_focus_events_enter_repo_mode_then_switch_subview() {
        let repo_id = RepoId::new("/tmp/repo");

        let status_events = startup_focus_events(repo_id.clone(), StartupFocus::Status);
        assert_eq!(status_events.len(), 1);
        assert!(matches!(
            status_events.first(),
            Some(Event::Action(Action::EnterRepoMode { repo_id: event_repo_id }))
                if event_repo_id == &repo_id
        ));

        let stash_events = startup_focus_events(repo_id.clone(), StartupFocus::Stash);
        assert_eq!(stash_events.len(), 2);
        assert!(matches!(
            stash_events.get(1),
            Some(Event::Action(Action::SwitchRepoSubview(RepoSubview::Stash)))
        ));

        let log_events = startup_focus_events(repo_id, StartupFocus::Log);
        assert!(matches!(
            log_events.get(1),
            Some(Event::Action(Action::SwitchRepoSubview(
                RepoSubview::Commits
            )))
        ));
    }

    #[test]
    fn resolve_workspace_root_prefers_cli_path_over_config_roots() {
        let config = AppConfig {
            workspace: super_lazygit_config::WorkspaceConfig {
                roots: vec![PathBuf::from("/config-root")],
                ..Default::default()
            },
            ..AppConfig::default()
        };

        assert_eq!(
            resolve_workspace_root(Some(PathBuf::from("/cli-root")), &config),
            Some(PathBuf::from("/cli-root"))
        );
    }

    #[test]
    fn resolve_workspace_root_uses_first_config_root_when_cli_is_absent() {
        let config = AppConfig {
            workspace: super_lazygit_config::WorkspaceConfig {
                roots: vec![PathBuf::from("/config-root"), PathBuf::from("/later-root")],
                ..Default::default()
            },
            ..AppConfig::default()
        };

        assert_eq!(
            resolve_workspace_root(None, &config),
            Some(PathBuf::from("/config-root"))
        );
    }

    #[test]
    fn cargo_metadata_exposes_short_and_compatibility_bins() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root");
        let output = Command::new("cargo")
            .args(["metadata", "--no-deps", "--format-version", "1"])
            .current_dir(workspace_root)
            .output()
            .expect("cargo metadata");
        assert!(
            output.status.success(),
            "cargo metadata should succeed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let metadata: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("cargo metadata json");
        let packages = metadata["packages"].as_array().expect("packages array");
        let app_package = packages
            .iter()
            .find(|package| package["name"] == "super-lazygit-app")
            .expect("app package");
        let targets = app_package["targets"].as_array().expect("targets array");
        let main_src = workspace_root.join("crates/app/src/main.rs");

        for expected_name in ["slg", "super-lazygit"] {
            let target = targets
                .iter()
                .find(|target| {
                    target["name"] == expected_name
                        && target["kind"]
                            .as_array()
                            .is_some_and(|kinds| kinds.iter().any(|kind| kind == "bin"))
                })
                .cloned();
            assert!(target.is_some(), "missing binary target {expected_name}");
            let target = target.expect("binary target should exist after assertion");
            assert_eq!(
                PathBuf::from(target["src_path"].as_str().expect("src path")),
                main_src,
                "{expected_name} should launch the shared app entrypoint"
            );
        }
    }

    #[test]
    fn short_launcher_script_targets_slg_bin() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root");
        let launcher = fs::read_to_string(workspace_root.join("slg")).expect("read slg launcher");
        assert!(
            launcher.contains("cargo run -p super-lazygit-app --bin slg --"),
            "launcher should target the short bin"
        );
    }

    #[test]
    fn lazygit_parity_regression_script_targets_documented_suites() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root");
        let script =
            fs::read_to_string(workspace_root.join("scripts/run_lazygit_parity_regression.sh"))
                .expect("read lazygit parity regression script");

        for expected in [
            "cargo fmt --all --check",
            "cargo check --all-targets",
            "cargo clippy --all-targets -- -D warnings",
            "cargo test -p super-lazygit-app lazygit_parity_regression_script_targets_documented_suites",
            "cargo test -p super-lazygit-app parity_matrix_lists_all_open_clone_parity_beads",
            "cargo test -p super-lazygit-tui route_repository_",
            "cargo test -p super-lazygit-tui repo_mode_",
            "cargo test -p super-lazygit-app e2e_keyboard_harness_runs_ -- --nocapture",
            "cargo test -p super-lazygit-app e2e_keyboard_harness_inspects_stash_files_before_applying_older_stash -- --nocapture",
            "cargo test -p super-lazygit-app runtime_enters_and_leaves_nested_submodule_repo -- --nocapture",
        ] {
            assert!(
                script.contains(expected),
                "parity regression script missing expected command {expected}"
            );
        }
    }

    #[test]
    fn parity_matrix_lists_all_open_clone_parity_beads() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root");
        let matrix =
            fs::read_to_string(workspace_root.join("docs/PARITY_MATRIX.md")).expect("read matrix");
        let issues =
            fs::read_to_string(workspace_root.join(".beads/issues.jsonl")).expect("read beads");

        let open_parity_ids = issues
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("valid issue json"))
            .filter(|issue| {
                matches!(issue["status"].as_str(), Some("open" | "in_progress"))
                    && issue["id"]
                        .as_str()
                        .is_some_and(|id| id.starts_with("slg-"))
                    && issue["external_ref"]
                        .as_str()
                        .is_some_and(|path| path.starts_with("./references/lazygit-master/"))
                    && issue["labels"].as_array().is_some_and(|labels| {
                        labels.iter().any(|label| label == "parity")
                            && labels.iter().any(|label| label == "upstream-lazygit")
                    })
            })
            .filter_map(|issue| issue["id"].as_str().map(str::to_string))
            .collect::<Vec<_>>();

        assert!(
            matrix.contains("Behavior parity complete ="),
            "parity matrix must define behavior parity completion"
        );
        assert!(
            matrix.contains("Source/test parity complete ="),
            "parity matrix must define source/test parity completion"
        );

        if open_parity_ids.is_empty() {
            assert!(
                matrix.contains("Current source/test parity status: complete."),
                "parity matrix must explicitly say when source/test parity is complete"
            );
            return;
        }

        assert!(
            matrix.contains("Current source/test parity status: incomplete;"),
            "parity matrix must explicitly say when source/test parity is incomplete"
        );
    }

    #[test]
    fn runtime_processes_effects_until_worker_events_update_state() {
        let config = AppConfig::default();
        let state = AppState::default();
        let app = TuiApp::new(state, config);
        let workspace = WorkspaceRegistry::new(None);
        let git = GitFacade::default();
        let mut runtime = AppRuntime::new(app, workspace, git);

        runtime.run([Event::Action(Action::RefreshVisibleRepos)]);

        assert_eq!(runtime.app().state().mode, AppMode::Workspace);
        assert!(matches!(
            runtime.app().state().workspace.scan_status,
            ScanStatus::Complete { .. }
        ));
    }

    #[test]
    fn bootstrap_hydrates_workspace_from_cache_before_scan_runs() {
        let root = tempfile::tempdir().expect("workspace root");
        let repo_path = root.path().join("repo-a");
        fs::create_dir_all(repo_path.join(".git")).expect("repo fixture");

        let repo_path = normalized_path(&repo_path);
        let repo_id = normalized_repo_id(&repo_path);
        let workspace_state = WorkspaceState {
            current_root: Some(root.path().to_path_buf()),
            discovered_repo_ids: vec![repo_id.clone()],
            repo_summaries: BTreeMap::from([(
                repo_id.clone(),
                RepoSummary {
                    repo_id: repo_id.clone(),
                    display_name: String::from("repo-a"),
                    real_path: repo_path.clone(),
                    display_path: repo_path.display().to_string(),
                    last_refresh_at: Some(Timestamp(9)),
                    ..RepoSummary::default()
                },
            )]),
            selected_repo_id: Some(repo_id.clone()),
            scan_status: ScanStatus::Complete { scanned_repos: 1 },
            last_full_refresh_at: Some(Timestamp(9)),
            ..WorkspaceState::default()
        };

        let workspace_writer = WorkspaceRegistry::new(Some(root.path().to_path_buf()));
        workspace_writer
            .persist_cache(&workspace_state)
            .expect("persist cache");

        let config = AppConfig::default();
        let state = AppState::default();
        let app = TuiApp::new(state, config);
        let workspace = WorkspaceRegistry::new(Some(root.path().to_path_buf()));
        let git = GitFacade::default();
        let mut runtime = AppRuntime::new(app, workspace, git);

        runtime.bootstrap().expect("bootstrap succeeds");

        assert_eq!(
            runtime.app().state().workspace.selected_repo_id,
            Some(repo_id.clone())
        );
        assert_eq!(
            runtime.app().state().workspace.discovered_repo_ids,
            vec![repo_id.clone()]
        );
        assert_eq!(
            runtime
                .app()
                .state()
                .workspace
                .repo_summaries
                .get(&repo_id)
                .map(|summary| summary.display_name.as_str()),
            Some("repo-a")
        );
    }

    #[test]
    fn runtime_refresh_batch_keeps_successes_when_one_repo_fails() {
        let repo = clean_repo().expect("fixture repo");
        let valid_repo_path = normalized_path(repo.path());
        let valid_repo_id = normalized_repo_id(&valid_repo_path);
        let invalid_repo_id =
            RepoId::new(valid_repo_path.join("missing-repo").display().to_string());

        let config = AppConfig::default();
        let state = AppState::default();
        let app = TuiApp::new(state, config);
        let workspace = WorkspaceRegistry::new(Some(repo.path().to_path_buf()));
        let git = GitFacade::default();
        let mut runtime = AppRuntime::new(app, workspace, git);

        runtime.run([Event::Worker(WorkerEvent::RepoScanCompleted {
            root: Some(repo.path().to_path_buf()),
            repo_ids: vec![valid_repo_id.clone(), invalid_repo_id.clone()],
            scanned_at: Timestamp(7),
        })]);

        let state = runtime.app().state();
        assert!(state.workspace.repo_summaries.contains_key(&valid_repo_id));
        assert!(state.background_jobs.values().any(|job| {
            job.target_repo.as_ref() == Some(&valid_repo_id)
                && matches!(job.state, BackgroundJobState::Succeeded)
        }));
        assert!(state.background_jobs.values().any(|job| {
            job.target_repo.as_ref() == Some(&invalid_repo_id)
                && matches!(job.state, BackgroundJobState::Failed { .. })
        }));
    }

    #[test]
    fn runtime_configures_watcher_and_marks_health_healthy() {
        let repo = clean_repo().expect("fixture repo");
        let repo_id = normalized_repo_id(repo.path());
        let handle = ScriptedWatcherHandle::new();

        let config = AppConfig::default();
        let state = AppState::default();
        let app = TuiApp::new(state, config);
        let workspace = WorkspaceRegistry::new(Some(repo.path().to_path_buf()));
        let git = GitFacade::default();
        let mut runtime = AppRuntime::with_watcher(
            app,
            workspace,
            git,
            ScriptedWatcherBackend::new(handle.clone()),
        );

        runtime.run([Event::Worker(WorkerEvent::RepoScanCompleted {
            root: Some(repo.path().to_path_buf()),
            repo_ids: vec![repo_id.clone()],
            scanned_at: Timestamp(7),
        })]);

        assert_eq!(
            runtime.app().state().workspace.watcher_health,
            WatcherHealth::Healthy
        );
        assert_eq!(
            handle.registrations(),
            vec![watcher::WatchRegistration {
                repo_id,
                path: normalized_path(repo.path()),
            }]
        );
    }

    #[test]
    fn runtime_marks_watcher_health_degraded_when_configuration_fails() {
        let repo = clean_repo().expect("fixture repo");
        let repo_id = normalized_repo_id(repo.path());
        let handle = ScriptedWatcherHandle::new();
        handle.set_configure_error("watch backend unavailable");

        let config = AppConfig::default();
        let state = AppState::default();
        let app = TuiApp::new(state, config);
        let workspace = WorkspaceRegistry::new(Some(repo.path().to_path_buf()));
        let git = GitFacade::default();
        let mut runtime =
            AppRuntime::with_watcher(app, workspace, git, ScriptedWatcherBackend::new(handle));

        runtime.run([Event::Worker(WorkerEvent::RepoScanCompleted {
            root: Some(repo.path().to_path_buf()),
            repo_ids: vec![repo_id],
            scanned_at: Timestamp(7),
        })]);

        assert_eq!(
            runtime.app().state().workspace.watcher_health,
            WatcherHealth::Degraded {
                message: "watch backend unavailable".to_string(),
            }
        );
    }

    #[test]
    fn runtime_periodic_refresh_polls_when_watcher_is_degraded() {
        let repo = clean_repo().expect("fixture repo");
        let repo_id = normalized_repo_id(repo.path());
        let handle = ScriptedWatcherHandle::new();
        handle.set_configure_error("watch backend unavailable");

        let config = AppConfig::default();
        let state = AppState::default();
        let app = TuiApp::new(state, config);
        let workspace = WorkspaceRegistry::new(Some(repo.path().to_path_buf()));
        let git = GitFacade::default();
        let mut runtime =
            AppRuntime::with_watcher(app, workspace, git, ScriptedWatcherBackend::new(handle));

        runtime.run([Event::Worker(WorkerEvent::RepoScanCompleted {
            root: Some(repo.path().to_path_buf()),
            repo_ids: vec![repo_id.clone()],
            scanned_at: Timestamp(7),
        })]);

        std::fs::write(repo.path().join("fallback.txt"), "poll me").expect("write fallback file");
        runtime.run([Event::Timer(TimerEvent::PeriodicRefreshTick)]);

        let summary = runtime
            .app()
            .state()
            .workspace
            .repo_summaries
            .get(&repo_id)
            .expect("summary after fallback poll");
        assert!(summary.dirty);
        assert_eq!(
            runtime.app().state().workspace.watcher_health,
            WatcherHealth::Degraded {
                message: "watch backend unavailable".to_string(),
            }
        );
        assert_eq!(
            summary.watcher_freshness,
            super_lazygit_core::WatcherFreshness::Fresh
        );
    }

    #[test]
    fn runtime_drains_repo_invalidations_from_watcher_backend() {
        let repo = clean_repo().expect("fixture repo");
        let repo_id = normalized_repo_id(repo.path());
        let handle = ScriptedWatcherHandle::new();
        handle.push_event(AppWatcherEvent::RepoInvalidated {
            repo_id: repo_id.clone(),
        });
        handle.push_event(AppWatcherEvent::RepoInvalidated {
            repo_id: repo_id.clone(),
        });

        let config = AppConfig::default();
        let state = AppState::default();
        let app = TuiApp::new(state, config);
        let workspace = WorkspaceRegistry::new(Some(repo.path().to_path_buf()));
        let git = GitFacade::default();
        let mut runtime =
            AppRuntime::with_watcher(app, workspace, git, ScriptedWatcherBackend::new(handle));

        runtime.run([Event::Action(Action::EnterRepoMode {
            repo_id: repo_id.clone(),
        })]);

        let state = runtime.app().state();
        assert!(state.workspace.repo_summaries.contains_key(&repo_id));
        assert!(state
            .repo_mode
            .as_ref()
            .and_then(|repo_mode| repo_mode.detail.as_ref())
            .is_some());
        assert!(state.background_jobs.values().any(|job| {
            job.target_repo.as_ref() == Some(&repo_id)
                && matches!(job.state, BackgroundJobState::Succeeded)
        }));
    }

    #[test]
    fn runtime_debounces_watcher_invalidations_into_single_refresh_cycle() {
        let repo = clean_repo().expect("fixture repo");
        let repo_id = normalized_repo_id(repo.path());
        let handle = ScriptedWatcherHandle::new();

        let config = AppConfig::default();
        let state = AppState::default();
        let app = TuiApp::new(state, config);
        let workspace = WorkspaceRegistry::new(Some(repo.path().to_path_buf()));
        let git = GitFacade::default();
        let mut runtime = AppRuntime::with_watcher(
            app,
            workspace,
            git,
            ScriptedWatcherBackend::new(handle.clone()),
        );

        runtime.run([Event::Action(Action::EnterRepoMode {
            repo_id: repo_id.clone(),
        })]);

        handle.push_event(AppWatcherEvent::RepoInvalidated {
            repo_id: repo_id.clone(),
        });
        handle.push_event(AppWatcherEvent::RepoInvalidated {
            repo_id: repo_id.clone(),
        });

        runtime.run(std::iter::empty());

        let state = runtime.app().state();
        let repo_refresh_jobs = state
            .background_jobs
            .values()
            .filter(|job| {
                job.target_repo.as_ref() == Some(&repo_id)
                    && matches!(job.kind, BackgroundJobKind::RepoRefresh)
                    && matches!(job.state, BackgroundJobState::Succeeded)
            })
            .count();
        assert_eq!(repo_refresh_jobs, 1);
        assert!(state.workspace.pending_watcher_invalidations.is_empty());
        assert!(!state.workspace.watcher_debounce_pending);
        assert!(state
            .repo_mode
            .as_ref()
            .and_then(|repo_mode| repo_mode.detail.as_ref())
            .is_some());

        let diagnostics = runtime.diagnostics_snapshot();
        assert!(diagnostics
            .watcher_events
            .iter()
            .any(|event| { event.kind == WatcherEventKind::Burst && event.path_count == 2 }));
    }

    #[test]
    fn runtime_drains_health_events_from_watcher_backend() {
        let handle = ScriptedWatcherHandle::new();

        let config = AppConfig::default();
        let state = AppState::default();
        let app = TuiApp::new(state, config);
        let workspace = WorkspaceRegistry::new(None);
        let git = GitFacade::default();
        let mut runtime = AppRuntime::with_watcher(
            app,
            workspace,
            git,
            ScriptedWatcherBackend::new(handle.clone()),
        );

        handle.push_event(AppWatcherEvent::WatcherDegraded {
            message: "watch lag".to_string(),
        });
        runtime.run(std::iter::empty());
        assert_eq!(
            runtime.app().state().workspace.watcher_health,
            WatcherHealth::Degraded {
                message: "watch lag".to_string(),
            }
        );

        handle.push_event(AppWatcherEvent::WatcherRecovered);
        runtime.run(std::iter::empty());
        assert_eq!(
            runtime.app().state().workspace.watcher_health,
            WatcherHealth::Healthy
        );
    }

    #[test]
    fn runtime_reconfigures_watcher_after_degraded_scan_and_recovers_health() {
        let repo = clean_repo().expect("fixture repo");
        let repo_id = normalized_repo_id(repo.path());
        let handle = ScriptedWatcherHandle::new();
        handle.set_configure_error("watch backend unavailable");

        let config = AppConfig::default();
        let state = AppState::default();
        let app = TuiApp::new(state, config);
        let workspace = WorkspaceRegistry::new(Some(repo.path().to_path_buf()));
        let git = GitFacade::default();
        let mut runtime = AppRuntime::with_watcher(
            app,
            workspace,
            git,
            ScriptedWatcherBackend::new(handle.clone()),
        );

        runtime.run([Event::Worker(WorkerEvent::RepoScanCompleted {
            root: Some(repo.path().to_path_buf()),
            repo_ids: vec![repo_id.clone()],
            scanned_at: Timestamp(7),
        })]);

        assert_eq!(
            runtime.app().state().workspace.watcher_health,
            WatcherHealth::Degraded {
                message: "watch backend unavailable".to_string(),
            }
        );
        assert!(handle.registrations().is_empty());

        runtime.run([Event::Worker(WorkerEvent::RepoScanCompleted {
            root: Some(repo.path().to_path_buf()),
            repo_ids: vec![repo_id.clone()],
            scanned_at: Timestamp(8),
        })]);

        assert_eq!(
            runtime.app().state().workspace.watcher_health,
            WatcherHealth::Healthy
        );
        assert_eq!(
            handle.registrations(),
            vec![watcher::WatchRegistration {
                repo_id,
                path: normalized_path(repo.path()),
            }]
        );
        assert!(runtime
            .diagnostics_snapshot()
            .watcher_events
            .iter()
            .any(|event| event.kind == WatcherEventKind::Created && event.path_count == 1));
    }

    #[test]
    fn runtime_surfaces_pull_failure_without_upstream() {
        let repo = clean_repo().expect("fixture repo");
        let repo_id = normalized_repo_id(repo.path());

        let config = AppConfig::default();
        let state = AppState::default();
        let app = TuiApp::new(state, config);
        let workspace = WorkspaceRegistry::new(Some(repo.path().to_path_buf()));
        let git = GitFacade::default();
        let mut runtime = AppRuntime::new(app, workspace, git);

        runtime.run([
            Event::Action(Action::EnterRepoMode {
                repo_id: repo_id.clone(),
            }),
            Event::Action(Action::PullCurrentBranch),
            Event::Action(Action::ConfirmPendingOperation),
        ]);

        let state = runtime.app().state();
        assert_eq!(
            state
                .notifications
                .back()
                .map(|notification| notification.text.as_str()),
            Some("git operation failed: pull requires an upstream tracking branch")
        );
        assert_eq!(
            state
                .repo_mode
                .as_ref()
                .map(|repo_mode| &repo_mode.operation_progress),
            Some(&super_lazygit_core::OperationProgress::Idle)
        );
        assert!(state
            .background_jobs
            .values()
            .any(|job| matches!(job.state, BackgroundJobState::Failed { .. })));
    }

    #[test]
    fn e2e_keyboard_harness_runs_workspace_triage_commit_and_push_flow() {
        let remote = TempRepo::bare().expect("remote fixture");
        let seed = TempRepo::new().expect("seed fixture");
        seed.write_file("tracked.txt", "base\n")
            .expect("write tracked file");
        seed.commit_all("initial").expect("seed initial commit");
        seed.add_remote("origin", remote.path())
            .expect("attach remote");
        seed.push("origin", "HEAD:main").expect("seed push");

        let repo = TempRepo::clone_from(remote.path()).expect("clone fixture");
        repo.git(["branch", "--set-upstream-to=origin/main", "main"])
            .expect("set upstream");
        repo.append_file("tracked.txt", "local change\n")
            .expect("make local change");

        let repo_id = normalized_repo_id(repo.path());
        let mut harness = E2eHarness::new(repo.path().to_path_buf());
        harness.bootstrap();
        harness.assert_state(
            |state| state.workspace.selected_repo_id.as_ref() == Some(&repo_id),
            "workspace selected repo should match scanned fixture",
        );
        harness.assert_latest_contains("Workspace");

        harness.press("enter repo mode", "enter");
        harness.assert_state(
            |state| state.mode == AppMode::Repository,
            "enter should switch into repo mode",
        );
        harness.assert_latest_contains("Repository shell");
        harness.assert_latest_contains("tracked.txt");

        harness.press("stage selected file", "space");
        assert_eq!(
            repo_status_without_app_cache(&repo).expect("status after staging"),
            "M  tracked.txt",
            "expected the selected file to be staged\n{}",
            harness.timeline()
        );
        harness.assert_state(
            |state| {
                state
                    .status_messages
                    .back()
                    .map(|message| message.text.as_str())
                    == Some("Staged tracked.txt")
            },
            "staging should emit an actionable status message",
        );

        harness.press("focus staged pane", "tab");
        harness.press("open commit box", "c");
        harness.assert_latest_contains("Commit box");

        harness.paste("paste commit message", "feat: e2e flow");
        harness.assert_latest_contains("feat: e2e flow");

        harness.press("submit commit", "enter");
        assert_eq!(
            repo_status_without_app_cache(&repo).expect("status after commit"),
            "",
            "expected the working tree to be clean after commit\n{}",
            harness.timeline()
        );
        assert_eq!(
            command_stdout(&repo, ["log", "-1", "--format=%s"]).expect("head subject"),
            "feat: e2e flow",
            "expected the staged commit to land with the typed message\n{}",
            harness.timeline()
        );
        harness.assert_state(
            |state| {
                state
                    .status_messages
                    .back()
                    .map(|message| message.text.as_str())
                    .is_some_and(|message| message.starts_with("Committed staged changes"))
            },
            "commit should update the repo status log",
        );

        harness.press("open push confirmation", "P");
        harness.assert_latest_contains("Confirm push");
        harness.assert_latest_contains("Enter or y confirms");

        harness.press("confirm push", "enter");
        harness.assert_state(
            |state| {
                state
                    .status_messages
                    .back()
                    .map(|message| message.text.as_str())
                    == Some("Pushed current branch")
            },
            "push should complete through the confirmation modal",
        );
        assert_eq!(
            command_stdout(&repo, ["rev-list", "--count", "origin/main..HEAD"])
                .expect("ahead count"),
            "0",
            "expected the local branch to be fully pushed\n{}",
            harness.timeline()
        );

        let pushed_clone = TempRepo::clone_from(remote.path()).expect("clone pushed remote");
        assert_eq!(
            command_stdout(&pushed_clone, ["log", "-1", "--format=%s"])
                .expect("pushed remote head"),
            "feat: e2e flow",
            "expected the remote head to include the e2e commit\n{}",
            harness.timeline()
        );
    }

    #[test]
    fn e2e_keyboard_harness_runs_repo_detail_filter_worktree_and_return_cycle() {
        let remote = TempRepo::bare().expect("remote fixture");
        let seed = TempRepo::new().expect("seed fixture");
        seed.write_file("tracked.txt", "base\n")
            .expect("write tracked file");
        seed.commit_all("initial").expect("seed initial commit");
        seed.add_remote("origin", remote.path())
            .expect("attach remote");
        seed.push("origin", "HEAD:main").expect("seed push");

        let repo = TempRepo::clone_from(remote.path()).expect("clone fixture");
        repo.git(["branch", "feature-contract"])
            .expect("create feature branch");
        let worktree_root = tempfile::tempdir().expect("worktree root");
        let worktree_path = worktree_root.path().join("repo-feature-contract");
        repo.git([
            "worktree",
            "add",
            worktree_path.to_str().expect("utf8 path"),
            "feature-contract",
        ])
        .expect("create feature worktree");
        let normalized_worktree_path = normalized_path(&worktree_path);
        std::fs::write(worktree_path.join("tracked.txt"), "feature branch only\n")
            .expect("update feature worktree file");
        git_in(&worktree_path, ["add", "tracked.txt"]);
        git_in(&worktree_path, ["commit", "-m", "feature branch commit"]);
        repo.git(["checkout", "main"])
            .expect("return main checkout after feature commit");

        let repo_id = normalized_repo_id(repo.path());
        let mut harness = E2eHarness::new(repo.path().to_path_buf());
        harness.bootstrap();
        harness.assert_state(
            |state| state.workspace.selected_repo_id.as_ref() == Some(&repo_id),
            "workspace selected repo should match scanned fixture",
        );

        harness.press("enter repo mode", "enter");
        harness.assert_state(
            |state| state.mode == AppMode::Repository,
            "enter should switch into repo mode",
        );
        harness.assert_latest_contains("Repository shell");

        harness.press("open branches detail", "2");
        harness.assert_latest_contains("Detail: Branches");
        harness.assert_latest_contains("Context: Enter commits. Space check");

        harness.press("focus branches filter", "/");
        harness.paste("filter branches", "fea");
        harness.assert_latest_contains("Filter /fea_");
        harness.assert_state(
            |state| {
                state.repo_mode.as_ref().is_some_and(|repo_mode| {
                    let selected_branch = repo_mode
                        .branches_view
                        .selected_index
                        .and_then(|index| {
                            repo_mode
                                .detail
                                .as_ref()
                                .and_then(|detail| detail.branches.get(index))
                        })
                        .map(|branch| branch.name.as_str());
                    repo_mode.branches_filter.focused
                        && repo_mode.branches_filter.query == "fea"
                        && selected_branch == Some("feature-contract")
                })
            },
            "filtering branches should focus the contextual query and reselect the matching row",
        );

        harness.press("blur branches filter", "enter");
        harness.assert_state(
            |state| {
                state.repo_mode.as_ref().is_some_and(|repo_mode| {
                    !repo_mode.branches_filter.focused && repo_mode.branches_filter.query == "fea"
                })
            },
            "enter should keep the query while leaving the filter field",
        );

        harness.press("open selected branch commits", "enter");
        harness.assert_latest_contains("Detail: Commits");
        harness.assert_latest_contains("feature branch commit");
        harness.assert_state(
            |state| {
                state.repo_mode.as_ref().is_some_and(|repo_mode| {
                    repo_mode.active_subview == RepoSubview::Commits
                        && repo_mode.commit_history_ref.as_deref() == Some("feature-contract")
                })
            },
            "enter from branches should drill into the selected branch history",
        );

        harness.press("reset commits view to current branch history", "3");
        harness.assert_state(
            |state| {
                state.repo_mode.as_ref().is_some_and(|repo_mode| {
                    repo_mode.active_subview == RepoSubview::Commits
                        && repo_mode.commit_history_mode == CommitHistoryMode::Linear
                        && repo_mode.commit_history_ref.is_none()
                })
            },
            "3 from an explicit branch history should reset the commits pane to the current branch",
        );

        harness.press("return to branches detail", "2");
        harness.assert_latest_contains("Detail: Branches");

        harness.press("open worktrees detail", "w");
        harness.assert_latest_contains("Detail: Worktrees");
        harness.assert_latest_contains("repo-feature-contract");

        harness.press("focus worktree filter", "/");
        harness.paste("filter worktrees", "feature");
        harness.assert_latest_contains("Filter /feature_");
        harness.assert_state(
            |state| {
                state.repo_mode.as_ref().is_some_and(|repo_mode| {
                    let selected_worktree = repo_mode
                        .worktree_view
                        .selected_index
                        .and_then(|index| {
                            repo_mode
                                .detail
                                .as_ref()
                                .and_then(|detail| detail.worktrees.get(index))
                        })
                        .map(|worktree| normalized_path(&worktree.path));
                    repo_mode.worktree_filter.focused
                        && repo_mode.worktree_filter.query == "feature"
                        && selected_worktree.as_deref() == Some(normalized_worktree_path.as_path())
                })
            },
            "filtering worktrees should focus the contextual query and reselect the matching row",
        );

        harness.press("blur worktree filter", "enter");
        harness.assert_state(
            |state| {
                state.repo_mode.as_ref().is_some_and(|repo_mode| {
                    !repo_mode.worktree_filter.focused
                        && repo_mode.worktree_filter.query == "feature"
                })
            },
            "enter should keep the query while leaving the worktree filter field",
        );

        harness.press("return to main pane", "0");
        harness.assert_state(
            |state| state.focused_pane == PaneId::RepoUnstaged,
            "0 should return focus to the main repo pane without leaving repo mode",
        );
        harness.assert_latest_contains("Working tree");
    }

    #[test]
    fn e2e_keyboard_harness_runs_remote_branch_commit_and_checkout_cycle() {
        let remote = TempRepo::bare().expect("remote fixture");
        let seed = TempRepo::new().expect("seed fixture");
        seed.write_file("tracked.txt", "base\n")
            .expect("write tracked file");
        seed.commit_all("initial").expect("seed initial commit");
        seed.add_remote("origin", remote.path())
            .expect("attach remote");
        seed.push("origin", "HEAD:main").expect("seed push main");
        seed.checkout_new_branch("feature-remote")
            .expect("create feature branch");
        seed.write_file("feature.txt", "remote branch content\n")
            .expect("write remote feature file");
        seed.commit_all("remote feature commit")
            .expect("commit remote feature branch");
        seed.push("origin", "HEAD:feature-remote")
            .expect("push feature branch");

        let repo = TempRepo::clone_from(remote.path()).expect("clone fixture");
        repo.git(["branch", "--set-upstream-to=origin/main", "main"])
            .expect("set upstream");

        let repo_id = normalized_repo_id(repo.path());
        let mut harness = E2eHarness::new(repo.path().to_path_buf());
        harness.bootstrap();
        harness.assert_state(
            |state| state.workspace.selected_repo_id.as_ref() == Some(&repo_id),
            "workspace selected repo should match scanned fixture",
        );

        harness.press("enter repo mode", "enter");
        harness.assert_state(
            |state| state.mode == AppMode::Repository,
            "enter should switch into repo mode",
        );

        harness.press("open remote branches detail", "9");
        harness.assert_latest_contains("Detail: Remote Branches");
        harness.assert_latest_contains("Context: Enter commits. Space check");

        harness.press("focus remote branch filter", "/");
        harness.paste("filter remote branches", "feature");
        harness.assert_latest_contains("Filter /feature_");
        harness.assert_state(
            |state| {
                state.repo_mode.as_ref().is_some_and(|repo_mode| {
                    let selected_branch = repo_mode
                        .remote_branches_view
                        .selected_index
                        .and_then(|index| {
                            repo_mode
                                .detail
                                .as_ref()
                                .and_then(|detail| detail.remote_branches.get(index))
                        })
                        .map(|branch| branch.name.as_str());
                    repo_mode.remote_branches_filter.focused
                        && repo_mode.remote_branches_filter.query == "feature"
                        && selected_branch == Some("origin/feature-remote")
                })
            },
            "filtering remote branches should focus the contextual query and reselect the matching row",
        );

        harness.press("blur remote branch filter", "enter");
        harness.press("open selected remote branch commits", "enter");
        harness.assert_latest_contains("Detail: Commits");
        harness.assert_latest_contains("remote feature commit");
        harness.assert_state(
            |state| {
                state.repo_mode.as_ref().is_some_and(|repo_mode| {
                    repo_mode.active_subview == RepoSubview::Commits
                        && repo_mode.commit_history_ref.as_deref() == Some("origin/feature-remote")
                })
            },
            "enter from remote branches should drill into the selected remote branch history",
        );

        harness.press("return to remote branches detail", "9");
        harness.press("checkout selected remote branch", "space");
        assert_eq!(
            repo.current_branch()
                .expect("current branch after checkout"),
            "feature-remote",
            "space from remote branches should create and check out the tracking branch\n{}",
            harness.timeline()
        );
        assert_eq!(
            command_stdout(
                &repo,
                ["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
            )
            .expect("tracking upstream"),
            "origin/feature-remote",
            "expected the checked out local branch to track the selected remote ref\n{}",
            harness.timeline()
        );
    }

    #[test]
    fn e2e_keyboard_harness_runs_remote_management_cycle() {
        let origin = TempRepo::bare().expect("origin fixture");
        let upstream = TempRepo::bare().expect("upstream fixture");
        let seed = TempRepo::new().expect("seed fixture");
        seed.write_file("tracked.txt", "base\n")
            .expect("write tracked file");
        seed.commit_all("initial").expect("seed initial commit");
        seed.add_remote("origin", origin.path())
            .expect("attach origin");
        seed.push("origin", "HEAD:main").expect("seed push main");
        seed.checkout_new_branch("feature-remote")
            .expect("create feature branch");
        seed.write_file("feature.txt", "remote branch content\n")
            .expect("write remote feature file");
        seed.commit_all("remote feature commit")
            .expect("commit remote feature branch");
        seed.push("origin", "HEAD:feature-remote")
            .expect("push feature branch");

        let upstream_seed = TempRepo::new().expect("upstream seed fixture");
        upstream_seed
            .write_file("upstream.txt", "upstream base\n")
            .expect("write upstream file");
        upstream_seed
            .commit_all("upstream initial")
            .expect("upstream initial commit");
        upstream_seed
            .add_remote("origin", upstream.path())
            .expect("attach upstream remote");
        upstream_seed
            .push("origin", "HEAD:main")
            .expect("push upstream main");

        let repo = TempRepo::clone_from(origin.path()).expect("clone fixture");
        repo.git(["branch", "--set-upstream-to=origin/main", "main"])
            .expect("set upstream");

        let repo_id = normalized_repo_id(repo.path());
        let mut harness = E2eHarness::new(repo.path().to_path_buf());
        harness.bootstrap();
        harness.assert_state(
            |state| state.workspace.selected_repo_id.as_ref() == Some(&repo_id),
            "workspace selected repo should match scanned fixture",
        );

        harness.press("enter repo mode", "enter");
        harness.assert_state(
            |state| state.mode == AppMode::Repository,
            "enter should switch into repo mode",
        );

        harness.press("open remotes detail", "m");
        harness.assert_latest_contains("Detail: Remotes");
        harness.assert_latest_contains("Context: Enter branches. f fetch.");

        harness.press("focus remotes filter", "/");
        harness.paste("filter remotes", "orig");
        harness.assert_latest_contains("Filter /orig_");
        harness.assert_state(
            |state| {
                state.repo_mode.as_ref().is_some_and(|repo_mode| {
                    let selected_remote = repo_mode
                        .remotes_view
                        .selected_index
                        .and_then(|index| {
                            repo_mode
                                .detail
                                .as_ref()
                                .and_then(|detail| detail.remotes.get(index))
                        })
                        .map(|remote| remote.name.as_str());
                    repo_mode.remotes_filter.focused
                        && repo_mode.remotes_filter.query == "orig"
                        && selected_remote == Some("origin")
                })
            },
            "filtering remotes should focus the query and keep the matching remote selected",
        );

        harness.press("blur remotes filter", "enter");
        harness.press("refocus remotes filter", "/");
        for _ in 0..4 {
            harness.press("clear remotes filter", "backspace");
        }
        harness.press("blur cleared remotes filter", "enter");
        harness.press("open create remote prompt", "n");
        harness.assert_latest_contains("Add remote");
        harness.paste(
            "type remote details",
            &format!("upstream {}", upstream.path().display()),
        );
        harness.press("submit create remote prompt", "enter");
        assert_eq!(
            command_stdout(&repo, ["remote", "get-url", "upstream"])
                .expect("upstream remote url after add"),
            upstream.path().display().to_string(),
            "adding a remote from the remotes panel should write the remote config\n{}",
            harness.timeline()
        );

        harness.press("focus remotes filter for upstream", "/");
        harness.paste("filter upstream remote", "upstream");
        harness.press("blur upstream filter", "enter");
        harness.assert_state(
            |state| {
                state.repo_mode.as_ref().is_some_and(|repo_mode| {
                    let selected_remote = repo_mode
                        .remotes_view
                        .selected_index
                        .and_then(|index| {
                            repo_mode
                                .detail
                                .as_ref()
                                .and_then(|detail| detail.remotes.get(index))
                        })
                        .map(|remote| remote.name.as_str());
                    selected_remote == Some("upstream")
                })
            },
            "filtering to the added remote should select it",
        );

        harness.press("open fetch remote confirmation", "f");
        harness.assert_latest_contains("Fetch remote upstream");
        harness.press("confirm fetch remote", "enter");
        assert_eq!(
            command_stdout(&repo, ["rev-parse", "refs/remotes/upstream/main"])
                .expect("upstream remote ref after fetch"),
            upstream_seed.rev_parse("HEAD").expect("upstream head"),
            "fetching the selected remote should update its tracking refs\n{}",
            harness.timeline()
        );

        harness.press("return to remotes detail after fetch", "m");
        harness.press("open edit remote prompt", "e");
        harness.assert_latest_contains("Edit remote upstream");
        let existing_prompt = format!("upstream {}", upstream.path().display());
        for _ in 0..existing_prompt.chars().count() {
            harness.press("clear edit remote prompt", "backspace");
        }
        harness.paste(
            "type edited remote details",
            &format!("mirror {}", origin.path().display()),
        );
        harness.press("submit edit remote prompt", "enter");
        assert_eq!(
            command_stdout(&repo, ["remote", "get-url", "mirror"])
                .expect("mirror remote url after edit"),
            origin.path().display().to_string(),
            "editing the selected remote should rename it and update its URL\n{}",
            harness.timeline()
        );
        repo.git_expect_failure(["remote", "get-url", "upstream"])
            .expect("old remote name should be gone after edit");

        harness.press("focus remotes filter for mirror", "/");
        for _ in 0..8 {
            harness.press("clear old remotes filter", "backspace");
        }
        harness.paste("filter renamed remote", "mirror");
        harness.press("blur mirror filter", "enter");
        harness.assert_state(
            |state| {
                state.repo_mode.as_ref().is_some_and(|repo_mode| {
                    let selected_remote = repo_mode
                        .remotes_view
                        .selected_index
                        .and_then(|index| {
                            repo_mode
                                .detail
                                .as_ref()
                                .and_then(|detail| detail.remotes.get(index))
                        })
                        .map(|remote| remote.name.as_str());
                    selected_remote == Some("mirror")
                })
            },
            "filtering to the renamed remote should select it",
        );

        harness.press("open remove remote confirmation", "d");
        harness.assert_latest_contains("Remove remote mirror");
        harness.press("confirm remove remote", "enter");
        repo.git_expect_failure(["remote", "get-url", "mirror"])
            .expect("mirror remote should be deleted");
    }

    #[test]
    fn e2e_keyboard_harness_runs_tag_filter_create_push_delete_cycle() {
        let remote = TempRepo::bare().expect("remote fixture");
        let seed = TempRepo::new().expect("seed fixture");
        seed.write_file("tracked.txt", "base\n")
            .expect("write tracked file");
        seed.commit_all("initial").expect("seed initial commit");
        seed.add_remote("origin", remote.path())
            .expect("attach remote");
        seed.push("origin", "HEAD:main").expect("seed push main");
        seed.git(["tag", "-a", "v1.0.0", "-m", "release v1.0.0"])
            .expect("create annotated tag");
        seed.push("origin", "refs/tags/v1.0.0")
            .expect("push annotated tag");

        let repo = TempRepo::clone_from(remote.path()).expect("clone fixture");
        repo.git(["branch", "--set-upstream-to=origin/main", "main"])
            .expect("set upstream");

        let repo_id = normalized_repo_id(repo.path());
        let mut harness = E2eHarness::new(repo.path().to_path_buf());
        harness.bootstrap();
        harness.assert_state(
            |state| state.workspace.selected_repo_id.as_ref() == Some(&repo_id),
            "workspace selected repo should match scanned fixture",
        );

        harness.press("enter repo mode", "enter");
        harness.assert_state(
            |state| state.mode == AppMode::Repository,
            "enter should switch into repo mode",
        );

        harness.press("open tags detail", "t");
        harness.assert_latest_contains("Detail: Tags");
        harness.assert_latest_contains("Context: Enter commits. Space check");

        harness.press("focus tag filter", "/");
        harness.paste("filter tags", "v1");
        harness.assert_latest_contains("Filter /v1_");
        harness.assert_state(
            |state| {
                state.repo_mode.as_ref().is_some_and(|repo_mode| {
                    let selected_tag = repo_mode
                        .tags_view
                        .selected_index
                        .and_then(|index| {
                            repo_mode
                                .detail
                                .as_ref()
                                .and_then(|detail| detail.tags.get(index))
                        })
                        .map(|tag| tag.name.as_str());
                    repo_mode.tags_filter.focused
                        && repo_mode.tags_filter.query == "v1"
                        && selected_tag == Some("v1.0.0")
                })
            },
            "filtering tags should focus the contextual query and reselect the matching row",
        );

        harness.press("blur tag filter", "enter");
        harness.press("open selected tag commits", "enter");
        harness.assert_latest_contains("Detail: Commits");
        harness.assert_latest_contains("initial");
        harness.assert_state(
            |state| {
                state.repo_mode.as_ref().is_some_and(|repo_mode| {
                    repo_mode.active_subview == RepoSubview::Commits
                        && repo_mode.commit_history_ref.as_deref() == Some("v1.0.0")
                })
            },
            "enter from tags should drill into the selected tag history",
        );

        harness.press("return to tags detail", "[");
        harness.press("checkout selected tag", "space");
        assert_eq!(
            command_stdout(&repo, ["rev-parse", "--abbrev-ref", "HEAD"]).expect("detached head"),
            "HEAD",
            "space from tags should check out the selected tag in detached HEAD mode\n{}",
            harness.timeline()
        );

        harness.press("open create tag prompt", "n");
        harness.assert_latest_contains("Create tag");
        harness.paste("type new tag name", "release-candidate");
        harness.press("submit create tag prompt", "enter");
        assert_eq!(
            command_stdout(&repo, ["tag", "--list", "release-candidate"]).expect("local tag list"),
            "release-candidate",
            "creating a tag from the tags panel should write the local ref\n{}",
            harness.timeline()
        );

        harness.press("focus tag filter again", "/");
        harness.press("clear first filter character", "backspace");
        harness.press("clear second filter character", "backspace");
        harness.paste("filter created tag", "release-candidate");
        harness.press("blur created tag filter", "enter");
        harness.assert_state(
            |state| {
                state.repo_mode.as_ref().is_some_and(|repo_mode| {
                    let selected_tag = repo_mode
                        .tags_view
                        .selected_index
                        .and_then(|index| {
                            repo_mode
                                .detail
                                .as_ref()
                                .and_then(|detail| detail.tags.get(index))
                        })
                        .map(|tag| tag.name.as_str());
                    selected_tag == Some("release-candidate")
                })
            },
            "filtering to the newly created tag should select it",
        );

        harness.press("open push tag confirmation", "P");
        harness.assert_latest_contains("Push tag release-candidate to origin");
        harness.press("confirm push tag", "enter");
        assert_eq!(
            command_stdout(&remote, ["tag", "--list", "release-candidate"])
                .expect("remote tag list"),
            "release-candidate",
            "pushing the selected tag should create it on the remote\n{}",
            harness.timeline()
        );

        harness.press("refocus tag detail", "t");
        harness.press("open delete tag confirmation", "d");
        harness.assert_latest_contains("Delete tag release-candidate");
        harness.press("confirm delete tag", "enter");
        assert_eq!(
            command_stdout(&repo, ["tag", "--list", "release-candidate"])
                .expect("local deleted tag"),
            "",
            "deleting the selected tag should remove the local ref\n{}",
            harness.timeline()
        );
    }

    #[test]
    fn e2e_keyboard_harness_runs_commit_history_file_and_detached_checkout_cycle() {
        let repo = TempRepo::new().expect("repo fixture");
        repo.write_file("src/lib.rs", "pub fn version() -> u8 {\n    1\n}\n")
            .expect("write initial source");
        repo.commit_all("initial commit")
            .expect("commit initial state");
        let initial_commit =
            command_stdout(&repo, ["rev-parse", "HEAD"]).expect("initial commit id");

        repo.write_file("src/lib.rs", "pub fn version() -> u8 {\n    2\n}\n")
            .expect("write updated source");
        repo.commit_all("second commit")
            .expect("commit updated state");

        let repo_id = normalized_repo_id(repo.path());
        let mut harness = E2eHarness::new(repo.path().to_path_buf());
        harness.bootstrap();
        harness.assert_state(
            |state| state.workspace.selected_repo_id.as_ref() == Some(&repo_id),
            "workspace selected repo should match scanned fixture",
        );

        harness.press("enter repo mode", "enter");
        harness.assert_state(
            |state| state.mode == AppMode::Repository,
            "enter should switch into repo mode",
        );

        harness.press("open commits detail", "3");
        harness.assert_latest_contains("Detail: Commits");
        harness.assert_latest_contains("Enter files");

        harness.press("open selected commit files", "enter");
        harness.assert_latest_contains("Commit files");
        harness.assert_latest_contains("src/lib.rs");

        harness.press("open selected commit file diff", "enter");
        harness.assert_latest_contains("Commit file diff");
        harness.assert_latest_contains("File: src/lib.rs");
        harness.assert_latest_contains("Path: src/lib.rs (comparison)");

        harness.press("return to commit files list", "enter");
        harness.assert_latest_contains("Commit files");

        harness.press("return to commit history", "backspace");
        harness.assert_latest_contains("Detail: Commits");

        harness.press("select older commit", "j");
        harness.assert_latest_contains("initial commit");

        harness.press("detached checkout selected commit", "space");
        harness.assert_state(
            |state| state.mode == AppMode::Repository,
            "detached checkout should keep the app in repo mode",
        );
        assert_eq!(
            command_stdout(&repo, ["rev-parse", "HEAD"]).expect("head after detached checkout"),
            initial_commit,
            "space from commit history should detach HEAD at the selected commit\n{}",
            harness.timeline()
        );
    }

    #[test]
    fn e2e_keyboard_harness_runs_reflog_history_and_detached_checkout_cycle() {
        let repo = TempRepo::new().expect("repo fixture");
        repo.write_file("src/lib.rs", "pub fn version() -> u8 {\n    1\n}\n")
            .expect("write initial source");
        repo.commit_all("initial commit")
            .expect("commit initial state");
        let initial_commit =
            command_stdout(&repo, ["rev-parse", "HEAD"]).expect("initial commit id");

        repo.write_file("src/lib.rs", "pub fn version() -> u8 {\n    2\n}\n")
            .expect("write updated source");
        repo.commit_all("second commit")
            .expect("commit updated state");

        let repo_id = normalized_repo_id(repo.path());
        let mut harness = E2eHarness::new(repo.path().to_path_buf());
        harness.bootstrap();
        harness.assert_state(
            |state| state.workspace.selected_repo_id.as_ref() == Some(&repo_id),
            "workspace selected repo should match scanned fixture",
        );

        harness.press("enter repo mode", "enter");
        harness.assert_state(
            |state| state.mode == AppMode::Repository,
            "enter should switch into repo mode",
        );

        harness.press("open reflog detail", "7");
        harness.assert_latest_contains("Detail: Reflog");
        harness.assert_latest_contains("Context: Enter commits. Space check");

        harness.press("select older reflog entry", "j");
        harness.assert_latest_contains("HEAD@{1}: commit (initial): initial");

        harness.press("open selected reflog entry in commits", "enter");
        harness.assert_latest_contains("Detail: Commits");
        harness.assert_latest_contains("commit (initial): initial");

        harness.press("return to reflog detail", "7");
        harness.press("detach checkout selected reflog target", "space");
        harness.assert_state(
            |state| state.mode == AppMode::Repository,
            "reflog detached checkout should keep the app in repo mode",
        );
        assert_eq!(
            command_stdout(&repo, ["rev-parse", "HEAD"]).expect("head after reflog checkout"),
            initial_commit,
            "space from reflog should detach HEAD at the selected target\n{}",
            harness.timeline()
        );
    }

    #[test]
    fn e2e_keyboard_harness_inspects_stash_files_before_applying_older_stash() {
        let repo = TempRepo::new().expect("repo fixture");
        repo.write_file("alpha.txt", "base\n")
            .expect("write alpha base");
        repo.write_file("beta.txt", "base\n")
            .expect("write beta base");
        repo.commit_all("initial commit")
            .expect("commit initial state");

        repo.write_file("alpha.txt", "alpha stash\n")
            .expect("write alpha stash");
        repo.git(["stash", "push", "-m", "alpha stash"])
            .expect("create alpha stash");

        repo.write_file("beta.txt", "beta stash\n")
            .expect("write beta stash");
        repo.git(["stash", "push", "-m", "beta stash"])
            .expect("create beta stash");

        let repo_id = normalized_repo_id(repo.path());
        let mut harness = E2eHarness::new(repo.path().to_path_buf());
        harness.bootstrap();
        harness.assert_state(
            |state| state.workspace.selected_repo_id.as_ref() == Some(&repo_id),
            "workspace selected repo should match scanned fixture",
        );

        harness.press("enter repo mode", "enter");
        harness.assert_state(
            |state| state.mode == AppMode::Repository,
            "enter should switch into repo mode",
        );

        harness.press("open stash detail", "6");
        harness.assert_latest_contains("Detail: Stash");
        harness.assert_latest_contains("Context: Enter files. Space apply.");

        harness.press("select older stash entry", "j");
        harness.assert_latest_contains("stash@{1}: On main: alpha stash");

        harness.press("open selected stash files", "enter");
        harness.assert_latest_contains("Stash files  stash@{1}  alpha stash");
        harness.assert_latest_contains("> M alpha.txt");

        harness.press("return to stash list", "enter");
        harness.assert_latest_contains("Context: Enter files. Space apply.");

        harness.press("apply selected stash", "space");
        harness.assert_state(
            |state| state.mode == AppMode::Repository,
            "applying a stash should keep the app in repo mode",
        );
        assert_eq!(
            fs::read_to_string(repo.path().join("alpha.txt"))
                .expect("read alpha after apply")
                .replace("\r\n", "\n"),
            "alpha stash\n",
            "space from the stash list should apply the selected stash after inspection\n{}",
            harness.timeline()
        );
    }

    #[test]
    fn performance_harness_measures_startup_scan_refresh_and_diff_loads() {
        let fixture = PerfWorkspaceFixture::new(6);
        let budgets = PerfBudgets::from_env();

        let mut cold = performance_runtime(fixture.root().to_path_buf());
        let cold_started = Instant::now();
        let cold_bootstrap = cold.bootstrap().expect("cold bootstrap");
        let cold_wall = cold_started.elapsed();

        let scan_before = cold.diagnostics_snapshot();
        cold.run([Event::Action(Action::RefreshVisibleRepos)]);
        let scan_after = cold.diagnostics_snapshot();
        assert_eq!(
            cold.app().state().workspace.discovered_repo_ids.len(),
            fixture.repo_count,
            "workspace scan should discover every perf fixture repo"
        );
        let workspace_scan = latest_git_timing_since(&scan_before, &scan_after, "scan_workspace");

        let refresh_before = cold.diagnostics_snapshot();
        cold.run([Event::Action(Action::RefreshSelectedRepo)]);
        let refresh_after = cold.diagnostics_snapshot();
        let summary_refresh =
            latest_git_timing_since(&refresh_before, &refresh_after, "read_repo_summary");

        let detail_before = cold.diagnostics_snapshot();
        cold.run([Event::Action(Action::EnterRepoMode {
            repo_id: fixture.diff_repo_id.clone(),
        })]);
        let detail_after = cold.diagnostics_snapshot();
        let repo_detail_load =
            latest_git_timing_since(&detail_before, &detail_after, "read_repo_detail");

        cold.persist_cache().expect("persist warm cache");

        let mut warm = performance_runtime(fixture.root().to_path_buf());
        let warm_started = Instant::now();
        let warm_bootstrap = warm.bootstrap().expect("warm bootstrap");
        let warm_wall = warm_started.elapsed();
        assert_eq!(
            warm.app().state().workspace.discovered_repo_ids.len(),
            fixture.repo_count,
            "warm startup should hydrate repo identities from cache"
        );

        let report = PerfReport {
            repo_count: fixture.repo_count,
            cold_startup_total: cold_bootstrap.startup_total,
            cold_startup_wall: cold_wall,
            warm_startup_total: warm_bootstrap.startup_total,
            warm_startup_wall: warm_wall,
            workspace_scan,
            summary_refresh,
            repo_detail_load,
        };
        eprintln!("{report}");

        assert!(
            report.cold_startup_wall <= budgets.cold_startup_wall,
            "cold startup wall time exceeded budget\nbudgets={budgets:?}\n{report}"
        );
        assert!(
            report.warm_startup_wall <= budgets.warm_startup_wall,
            "warm startup wall time exceeded budget\nbudgets={budgets:?}\n{report}"
        );
        assert!(
            report.workspace_scan <= budgets.workspace_scan,
            "workspace scan exceeded budget\nbudgets={budgets:?}\n{report}"
        );
        assert!(
            report.summary_refresh <= budgets.summary_refresh,
            "summary refresh exceeded budget\nbudgets={budgets:?}\n{report}"
        );
        assert!(
            report.repo_detail_load <= budgets.repo_detail_load,
            "repo detail load exceeded budget\nbudgets={budgets:?}\n{report}"
        );
    }

    struct E2eHarness {
        runtime: AppRuntime,
        timeline: Vec<String>,
    }

    impl E2eHarness {
        fn new(workspace_root: PathBuf) -> Self {
            let config = AppConfig::default();
            let state = AppState::default();
            let mut app = TuiApp::new(state, config);
            app.resize(120, 28);

            Self {
                runtime: AppRuntime::new(
                    app,
                    WorkspaceRegistry::new(Some(workspace_root)),
                    GitFacade::default(),
                ),
                timeline: Vec::new(),
            }
        }

        fn bootstrap(&mut self) {
            self.runtime.bootstrap().expect("bootstrap succeeds");
            self.snapshot("bootstrap");
            self.runtime
                .run([Event::Action(Action::RefreshVisibleRepos)]);
            self.snapshot("refresh visible repos");
        }

        fn press(&mut self, label: &str, key: &str) {
            self.runtime
                .run([Event::Input(super_lazygit_core::InputEvent::KeyPressed(
                    super_lazygit_core::KeyPress {
                        key: key.to_string(),
                    },
                ))]);
            self.snapshot(label);
        }

        fn paste(&mut self, label: &str, text: &str) {
            self.runtime
                .run([Event::Input(super_lazygit_core::InputEvent::Paste(
                    text.to_string(),
                ))]);
            self.snapshot(label);
        }

        #[track_caller]
        fn assert_latest_contains(&self, needle: &str) {
            let latest = self
                .timeline
                .last()
                .expect("timeline should contain at least one snapshot");
            assert!(
                latest.contains(needle),
                "expected latest snapshot to contain {needle:?}\n{}",
                self.timeline()
            );
        }

        #[track_caller]
        fn assert_state(&self, predicate: impl FnOnce(&AppState) -> bool, message: &str) {
            assert!(
                predicate(self.runtime.app().state()),
                "{message}\n{}",
                self.timeline()
            );
        }

        fn snapshot(&mut self, label: &str) {
            let frame = self.runtime.render_to_string();
            let state = self.runtime.app().state();
            let notifications = state
                .notifications
                .iter()
                .map(|notification| notification.text.as_str())
                .collect::<Vec<_>>();
            let status_messages = state
                .status_messages
                .iter()
                .map(|message| message.text.as_str())
                .collect::<Vec<_>>();
            let jobs = state
                .background_jobs
                .values()
                .map(|job| format!("{}:{:?}", job.id.0, job.state))
                .collect::<Vec<_>>();
            let operation_progress = state
                .repo_mode
                .as_ref()
                .map(|repo_mode| format!("{:?}", repo_mode.operation_progress))
                .unwrap_or_else(|| "None".to_string());
            let modal = state
                .modal_stack
                .last()
                .map(|modal| format!("{:?}", modal.kind))
                .unwrap_or_else(|| "None".to_string());

            self.timeline.push(format!(
                "=== {label} ===\nmode={:?} focus={:?} modal={modal} progress={operation_progress}\nstatus_messages={status_messages:?}\nnotifications={notifications:?}\njobs={jobs:?}\n{frame}",
                state.mode, state.focused_pane
            ));
        }

        fn timeline(&self) -> String {
            self.timeline.join("\n\n")
        }
    }

    fn command_stdout<const N: usize>(repo: &TempRepo, args: [&str; N]) -> std::io::Result<String> {
        String::from_utf8(repo.git_capture(args)?.stdout)
            .map(|value| value.trim().to_string())
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
    }

    fn repo_status_without_app_cache(repo: &TempRepo) -> std::io::Result<String> {
        Ok(repo
            .status_porcelain()?
            .lines()
            .filter(|line| *line != "?? .super-lazygit/")
            .collect::<Vec<_>>()
            .join("\n"))
    }

    fn performance_runtime(workspace_root: PathBuf) -> AppRuntime {
        let config = AppConfig::default();
        let state = AppState::default();
        let mut app = TuiApp::new(state, config);
        app.resize(120, 32);
        AppRuntime::new(
            app,
            WorkspaceRegistry::new(Some(workspace_root)),
            GitFacade::default(),
        )
    }

    #[test]
    fn open_in_editor_runs_configured_command_from_selected_repo_root() {
        let root = tempfile::tempdir().expect("workspace root");
        let repo_root = root.path().join("repo-a");
        fs::create_dir_all(repo_root.join(".git")).expect("repo fixture");
        fs::write(repo_root.join("README.md"), "fixture").expect("repo file");
        let log_path = root.path().join("editor.log");
        let repo_root = normalized_path(&repo_root);
        let repo_id = normalized_repo_id(&repo_root);

        let config = AppConfig {
            editor: super_lazygit_config::EditorConfig {
                command: "sh".to_string(),
                args: vec![
                    "-c".to_string(),
                    "printf '%s\n' \"$PWD\" > \"$1\"\nprintf '%s\n' \"$2\" >> \"$1\"".to_string(),
                    "editor-open".to_string(),
                    log_path.display().to_string(),
                ],
                ..Default::default()
            },
            ..AppConfig::default()
        };
        let state = AppState {
            workspace: WorkspaceState {
                discovered_repo_ids: vec![repo_id.clone()],
                repo_summaries: BTreeMap::from([(
                    repo_id.clone(),
                    RepoSummary {
                        repo_id: repo_id.clone(),
                        display_name: "repo-a".to_string(),
                        real_path: repo_root.clone(),
                        display_path: repo_root.display().to_string(),
                        ..RepoSummary::default()
                    },
                )]),
                selected_repo_id: Some(repo_id),
                ..WorkspaceState::default()
            },
            ..AppState::default()
        };
        let mut app = TuiApp::new(state, config);
        app.resize(120, 32);
        let mut runtime = AppRuntime::new(
            app,
            WorkspaceRegistry::new(Some(root.path().to_path_buf())),
            GitFacade::default(),
        );

        runtime.run([Event::Action(Action::OpenInEditor)]);

        let log = fs::read_to_string(&log_path).expect("editor log");
        let lines = log.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|line| line.ends_with("repo-a")));
    }

    fn latest_git_timing_since(
        before: &super_lazygit_core::DiagnosticsSnapshot,
        after: &super_lazygit_core::DiagnosticsSnapshot,
        operation_prefix: &str,
    ) -> Duration {
        after.git_operations[before.git_operations.len()..]
            .iter()
            .rev()
            .find(|timing| timing.operation.starts_with(operation_prefix))
            .map(|timing| timing.elapsed)
            .expect("git timing should exist for perf checkpoint")
    }

    #[derive(Debug)]
    struct PerfBudgets {
        cold_startup_wall: Duration,
        warm_startup_wall: Duration,
        workspace_scan: Duration,
        summary_refresh: Duration,
        repo_detail_load: Duration,
    }

    impl PerfBudgets {
        fn from_env() -> Self {
            let repo_detail_default_ms = if cfg!(windows) { 1500 } else { 1000 };
            Self {
                cold_startup_wall: duration_budget("SUPER_LAZYGIT_PERF_COLD_STARTUP_MS", 400),
                warm_startup_wall: duration_budget("SUPER_LAZYGIT_PERF_WARM_STARTUP_MS", 250),
                workspace_scan: duration_budget("SUPER_LAZYGIT_PERF_SCAN_MS", 2000),
                summary_refresh: duration_budget("SUPER_LAZYGIT_PERF_REFRESH_MS", 1000),
                repo_detail_load: duration_budget(
                    "SUPER_LAZYGIT_PERF_DETAIL_MS",
                    repo_detail_default_ms,
                ),
            }
        }
    }

    #[derive(Debug)]
    struct PerfReport {
        repo_count: usize,
        cold_startup_total: Duration,
        cold_startup_wall: Duration,
        warm_startup_total: Duration,
        warm_startup_wall: Duration,
        workspace_scan: Duration,
        summary_refresh: Duration,
        repo_detail_load: Duration,
    }

    impl std::fmt::Display for PerfReport {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "PERF REPORT repos={} cold_startup_total_ms={} cold_startup_wall_ms={} warm_startup_total_ms={} warm_startup_wall_ms={} workspace_scan_ms={} summary_refresh_ms={} repo_detail_load_ms={}",
                self.repo_count,
                self.cold_startup_total.as_millis(),
                self.cold_startup_wall.as_millis(),
                self.warm_startup_total.as_millis(),
                self.warm_startup_wall.as_millis(),
                self.workspace_scan.as_millis(),
                self.summary_refresh.as_millis(),
                self.repo_detail_load.as_millis(),
            )
        }
    }

    struct PerfWorkspaceFixture {
        root: tempfile::TempDir,
        diff_repo_id: RepoId,
        repo_count: usize,
    }

    impl PerfWorkspaceFixture {
        fn new(repo_count: usize) -> Self {
            let root = tempfile::tempdir().expect("perf workspace root");
            let mut diff_repo_id = None;

            for index in 0..repo_count {
                let repo_name = format!("repo-{index:02}");
                let repo_path = root.path().join(&repo_name);
                let diff_repo = index == 0;
                init_perf_repo(&repo_path, diff_repo);
                if diff_repo {
                    diff_repo_id = Some(repo_id_for_path(&repo_path));
                }
            }

            Self {
                root,
                diff_repo_id: diff_repo_id.expect("diff repo id"),
                repo_count,
            }
        }

        fn root(&self) -> &Path {
            self.root.path()
        }
    }

    fn init_perf_repo(path: &Path, with_unstaged_diff: bool) {
        fs::create_dir_all(path).expect("create perf repo dir");
        git_in(path, ["init", "--initial-branch=main"]);
        git_in(path, ["config", "user.name", "Super Lazygit Perf"]);
        git_in(path, ["config", "user.email", "perf@example.com"]);

        fs::write(path.join("README.md"), "# Perf Fixture\n").expect("write readme");
        if with_unstaged_diff {
            fs::create_dir_all(path.join("src")).expect("create src dir");
            fs::write(
                path.join("src/lib.rs"),
                "pub fn value() -> u32 {\n    1\n}\n",
            )
            .expect("write initial diff file");
        }

        git_in(path, ["add", "."]);
        git_in(path, ["commit", "-m", "initial"]);

        if with_unstaged_diff {
            fs::write(
                path.join("src/lib.rs"),
                "pub fn value() -> u32 {\n    2\n}\n\npub fn extra() -> u32 {\n    3\n}\n",
            )
            .expect("write updated diff file");
        }
    }

    fn git_in<const N: usize>(path: &Path, args: [&str; N]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(path)
            .output()
            .expect("run perf git command");
        assert!(
            output.status.success(),
            "git command failed in {} with args {:?}\nstdout:\n{}\nstderr:\n{}",
            path.display(),
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn repo_id_for_path(path: &Path) -> RepoId {
        RepoId::new(
            path.canonicalize()
                .unwrap_or_else(|_| path.to_path_buf())
                .display()
                .to_string(),
        )
    }

    fn duration_budget(name: &str, default_ms: u64) -> Duration {
        std::env::var(name)
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .map(Duration::from_millis)
            .unwrap_or_else(|| Duration::from_millis(default_ms))
    }
}
