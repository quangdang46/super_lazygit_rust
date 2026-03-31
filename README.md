# super_lazygit_rust

`super_lazygit_rust` is a workspace-first Git TUI written in Rust. The product goal is a Lazygit-grade single-repo experience inside a multi-repo workspace shell: fast repo discovery, clear triage signals, safe Git operations, and history/reference views that do not force you back to ad hoc shell commands.

## Current status

The repository now ships a real interactive terminal application when both stdin and stdout are attached to a TTY. Startup still bootstraps workspace state, cache, watcher health, and the initial refresh path before entering the long-running `crossterm` + `ratatui` loop.

That means:

- TTY launches stay open and run the interactive session
- non-TTY launches still bootstrap, refresh once, emit diagnostics when enabled, and exit
- reducer, runtime, Git, watcher, and TUI behavior are all covered by unit and integration tests

## What is implemented now

- Workspace discovery rooted at `--workspace <path>`, the first configured `workspace.roots` entry, or the current directory
- On-disk config loading from `SUPER_LAZYGIT_CONFIG`, `$XDG_CONFIG_HOME/super-lazygit/config.toml`, or `$HOME/.config/super-lazygit/config.toml`
- Workspace cache hydration under `.super-lazygit/workspace-cache.json`
- Workspace triage signals such as dirty/conflicted/ahead/behind counts
- Workspace search, filter, sort, and selection behavior
- Repo-mode shell with working tree, staged changes, and detail panes
- External editor handoff from the workspace repo list and repo status views
- File stage and unstage flows
- Commit, no-verify commit, and amend commit-box flows, plus Git-editor commit and reword flows
- Fetch, pull, and push flows with confirmation overlays
- Commit history detail view with compare-target context
- Status-origin all-branches commit graph view with forward and reverse ordering
- Branch detail view with checkout, create, rename, delete, upstream-setting, force-checkout, git-flow, pull-request, reset, sort, and tag-from-branch actions
- Remotes detail view with filtering, remote-branches drill-down, add/edit/remove/fetch actions, and fork-remote flow
- Remote-branch detail view with filtering, commit-history drill-down, tracking checkout, local-branch creation, remote deletion, pull-request, reset, sort, and tag-from-remote-branch actions
- Tags detail view with filtering, commit-history drill-down, detached checkout, copy/reset-menu affordances, and create/delete/push flows
- Compare, rebase, stash, reflog, and worktree detail subviews
- Submodules detail view with filtering, nested repo entry/return flow, copy/options affordances, and add/edit-url/init/update/remove actions
- Stash create/apply/pop/drop/rename/new-branch, stash-options menu parity for tracked, keep-index, include-untracked, staged, and lazygit-style unstaged creation, plus stash changed-file drill-down inspection before apply and reflog commit-history handoff, copy/browser/reset-menu affordances, detached checkout, branch-off, cherry-pick, restore, and reset flows
- Status detail shortcuts for config open/edit and release-page update checks
- Keybinding overrides for routed command keys across modal, workspace, repo, and commit-box actions
- Watcher health reporting with degraded fallback polling
- Fixture-heavy Git integration coverage for dirty repos, conflicts, upstream divergence, stashes, reflog, worktrees, and submodules

Current submodule scope now includes a lightweight options entry for the selected submodule. Broader multi-submodule batch operations still remain explicit rather than silently implied.

## Running the current build

```bash
./slg --workspace /path/to/workspace
```

The repo now ships a short local launcher for day-to-day iteration. It runs the same
`src/main.rs` entrypoint as the compatibility binary target and is the recommended
developer path from the repository root.

Compatibility paths stay valid:

```bash
cargo run -p super-lazygit-app -- --workspace /path/to/workspace
cargo run -p super-lazygit-app --bin super-lazygit -- --workspace /path/to/workspace
cargo run -p super-lazygit-app --bin slg -- --workspace /path/to/workspace
```

Workspace-root resolution order is:

1. `--workspace <path>`
2. the first configured `workspace.roots` entry
3. the current working directory

Config-file discovery order is:

1. `SUPER_LAZYGIT_CONFIG`
2. `$XDG_CONFIG_HOME/super-lazygit/config.toml`
3. `$HOME/.config/super-lazygit/config.toml`
4. built-in defaults

Interactive mode requires a TTY on both stdin and stdout. If you launch the binary in a pipeline or from a non-interactive harness, it will run startup/bootstrap logic and then exit after the initial refresh path.

## Verification

The current codebase is exercised primarily through:

```bash
cargo fmt --all
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --workspace
./scripts/run_lazygit_parity_regression.sh
```

## Docs

- [Lazygit Parity Matrix](docs/PARITY_MATRIX.md)
- [Keybindings](docs/KEYBINDINGS.md)
- [Config](docs/CONFIG.md)
- [Troubleshooting](docs/TROUBLESHOOTING.md)

## Architecture at a glance

- `crates/app`: binary entrypoint, runtime orchestration, TTY driver, and watcher plumbing
- `crates/core`: state, actions, reducer, effects, and domain structs
- `crates/tui`: ratatui rendering and key routing
- `crates/git`: Git facade and CLI-backed Git operations
- `crates/workspace`: workspace registry, scan bookkeeping, and cache persistence
- `crates/config`: config schema, discovery, and defaults
- `crates/test-support`: Git fixture builders used by integration tests
