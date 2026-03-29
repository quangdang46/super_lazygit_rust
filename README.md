# super_lazygit_rust

`super_lazygit_rust` is a workspace-first Git TUI written in Rust. The product goal is a Lazygit-grade single-repo experience inside a multi-repo workspace shell: fast repo discovery, clear triage signals, safe Git operations, and history/reference views that do not force you back to ad hoc shell commands.

## Current status

This repository already has substantial reducer, Git, watcher, cache, and TUI-shell coverage, but it is not yet a finished interactive terminal application. The binary currently bootstraps state, performs the initial refresh path, records diagnostics, and exits; the long-running `crossterm` input loop is not wired in yet.

That means:

- the repo contains the shell contract and key-routing behavior
- those behaviors are covered heavily by unit and integration tests
- the checked-in binary is still a foundation build, not a production-ready daily driver

## What is implemented now

- Workspace discovery rooted at `--workspace <path>` or the current directory
- Workspace cache hydration under `.super-lazygit/workspace-cache.json`
- Workspace triage signals such as dirty/conflicted/ahead/behind counts
- Workspace search, filter, sort, and selection behavior
- Repo-mode shell with working tree, staged changes, and detail panes
- File stage and unstage flows
- Commit and amend commit-box flows
- Fetch, pull, and push flows with confirmation overlays where appropriate
- Commit history detail view with compare-target context
- Branch detail view with checkout, create, rename, delete, and upstream-setting actions
- Watcher health reporting with degraded fallback polling
- Fixture-heavy Git integration coverage for dirty repos, conflicts, upstream divergence, stashes, reflog, and worktrees

## What is still partial

- No interactive terminal event loop yet
- `stash`, `reflog`, and `worktrees` subviews are present in the shell scaffold, but action flows are not complete
- The config schema exists, but the app currently uses `AppConfig::default()` directly instead of loading a config file from disk
- Keybinding override config exists as a schema surface, but overrides are not applied yet

## Running the current build

```bash
cargo run -p super-lazygit-app -- --workspace /path/to/workspace
```

If `--workspace` is omitted, the app falls back to the first configured workspace root if one exists in memory, otherwise the current working directory.

## Verification

The current codebase is exercised primarily through tests:

```bash
cargo test --workspace
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
```

## Docs

- [Keybindings](docs/KEYBINDINGS.md)
- [Config](docs/CONFIG.md)
- [Troubleshooting](docs/TROUBLESHOOTING.md)

## Architecture at a glance

- `crates/app`: binary entrypoint and runtime orchestration
- `crates/core`: state, actions, reducer, effects, and domain structs
- `crates/tui`: ratatui rendering and key routing
- `crates/git`: Git facade and CLI-backed Git operations
- `crates/workspace`: workspace registry, scan bookkeeping, and cache persistence
- `crates/config`: config schema and defaults
- `crates/test-support`: Git fixture builders used by integration tests
