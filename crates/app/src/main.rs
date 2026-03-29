use std::time::Instant;

use anyhow::Result;
use clap::Parser;

mod runtime;
mod watcher;

use runtime::AppRuntime;
use super_lazygit_config::AppConfig;
use super_lazygit_core::{Action, AppState, Diagnostics, Event};
use super_lazygit_git::GitFacade;
use super_lazygit_tui::TuiApp;
use super_lazygit_workspace::WorkspaceRegistry;

#[derive(Debug, Parser)]
#[command(name = "super-lazygit")]
#[command(about = "Workspace-first Lazygit-grade Rust TUI")]
struct Cli {
    /// Path to the workspace root to open.
    #[arg(long)]
    workspace: Option<std::path::PathBuf>,
}

fn main() -> Result<()> {
    let startup_started_at = Instant::now();
    let cli = Cli::parse();
    let config = AppConfig::default();
    let workspace_root = cli
        .workspace
        .or_else(|| config.workspace.roots.first().cloned())
        .or_else(|| std::env::current_dir().ok());
    let state = AppState::default();
    let app = TuiApp::new(state, config.clone());
    let workspace = WorkspaceRegistry::new(workspace_root);
    let git = GitFacade::default();

    let mut diagnostics = Diagnostics::default();

    let mut runtime = AppRuntime::new(app, workspace, git);
    diagnostics.extend_snapshot(runtime.bootstrap()?);
    runtime.run([Event::Action(Action::RefreshVisibleRepos)]);

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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;

    use super::watcher::{ScriptedWatcherBackend, ScriptedWatcherHandle};
    use super::*;
    use super_lazygit_core::{
        AppMode, AppWatcherEvent, BackgroundJobKind, BackgroundJobState, Event, RepoId,
        RepoSummary, ScanStatus, Timestamp, WatcherEventKind, WatcherHealth, WorkerEvent,
        WorkspaceState,
    };
    use super_lazygit_test_support::{clean_repo, TempRepo};

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

        let repo_id = RepoId::new(repo_path.display().to_string());
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
        let valid_repo_id = RepoId::new(repo.path().display().to_string());
        let invalid_repo_id = RepoId::new(repo.path().join("missing-repo").display().to_string());

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
        let repo_id = RepoId::new(repo.path().display().to_string());
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
                path: repo.path().to_path_buf(),
            }]
        );
    }

    #[test]
    fn runtime_marks_watcher_health_degraded_when_configuration_fails() {
        let repo = clean_repo().expect("fixture repo");
        let repo_id = RepoId::new(repo.path().display().to_string());
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
    fn runtime_drains_repo_invalidations_from_watcher_backend() {
        let repo = clean_repo().expect("fixture repo");
        let repo_id = RepoId::new(repo.path().display().to_string());
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
        let repo_id = RepoId::new(repo.path().display().to_string());
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
        let repo_id = RepoId::new(repo.path().display().to_string());
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
                path: repo.path().to_path_buf(),
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
        let repo_id = RepoId::new(repo.path().display().to_string());

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
            Some(&super_lazygit_core::OperationProgress::Failed {
                summary: "git operation failed: pull requires an upstream tracking branch"
                    .to_string(),
            })
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

        let repo_id = RepoId::new(repo.path().display().to_string());
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

        harness.press("stage selected file", "enter");
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
}
