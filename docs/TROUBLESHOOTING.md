# Troubleshooting

## The binary starts and exits immediately

That is the current expected behavior. The repo has the reducer, runtime, cache, watcher, Git, and TUI shell layers, but the long-running terminal input loop has not been wired into the application binary yet.

Use these commands to validate the current implementation:

```bash
cargo test --workspace
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
```

## I do not see any repositories

The app resolves the workspace root in this order:

1. `--workspace <path>`
2. the first configured workspace root in memory
3. the current working directory

Try pointing it at a directory that actually contains Git repositories:

```bash
cargo run -p super-lazygit-app -- --workspace /path/to/workspace
```

The discovery layer also ignores `.git`, `node_modules`, and `target` by default.

## Watch state says `polling`

`polling` means the watcher backend degraded and the runtime fell back to periodic refresh. That is a supported degraded mode, not a silent failure.

The status labels currently map like this:

- `unknown`: no watcher state established yet
- `live`: watcher backend configured successfully
- `polling`: watcher backend degraded and fallback polling is active

If you see `polling`, check the watcher error surfaced in diagnostics or tests and confirm the host can support file watching.

## Workspace state feels stale after startup

Workspace cache is stored under:

```text
.super-lazygit/workspace-cache.json
```

The cache is considered stale after five minutes. When a stale cache is loaded, repo summaries are marked stale until the runtime refreshes them.

If you want a clean bootstrap path, delete the cache directory:

```bash
rm -rf .super-lazygit
```

## Pull fails with an upstream-related error

That is expected when the current branch has no upstream tracking branch. The Git layer intentionally surfaces that failure instead of guessing.

Use regular Git to inspect or set the upstream if needed:

```bash
git branch -vv
git branch --set-upstream-to origin/<branch> <branch>
```

The repo-mode branch prompt is also designed to support upstream assignment once the interactive terminal driver is live.

## Branch delete is more forceful than plain `git branch -d`

The current delete-branch implementation uses `git branch -D` behind an explicit confirmation modal. That choice is deliberate: the softer `-d` path blocked confirmed-destructive flows in cases where upstream merge state prevented deletion even though the user had already confirmed the destructive action.

If you need the safer refusal semantics of `git branch -d`, use Git directly for now.

## I expected a config file to be loaded

The config schema exists, but the app currently does not parse an on-disk config file. The binary constructs `AppConfig::default()` directly.

See [CONFIG.md](CONFIG.md) for the current schema and defaults.

## I need deeper visibility into startup behavior

Diagnostics are enabled by default, and startup emits a stderr line when diagnostics logging is on. You can also use the test suite as a debugging surface because many workflows already have direct harness coverage:

- workspace bootstrap and cache hydration
- watcher degradation and recovery
- repo summary refresh behavior
- commit, push, pull, and branch lifecycle flows
- performance budgets for cold start, warm start, refresh, and detail loads
