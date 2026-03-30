# Troubleshooting

## The binary starts and exits immediately

The app only enters the interactive terminal loop when both stdin and stdout are attached to a TTY. If you run it in a pipeline, from a non-interactive harness, or under redirected IO, it will still bootstrap workspace state, run the initial refresh path, emit diagnostics when enabled, and then exit.

Use a normal terminal session for interactive mode:

```bash
cargo run -p super-lazygit-app -- --workspace /path/to/workspace
```

## I do not see any repositories

Workspace-root resolution order is:

1. `--workspace <path>`
2. the first configured `workspace.roots` entry
3. the current working directory

Config discovery order is:

1. `SUPER_LAZYGIT_CONFIG`
2. `$XDG_CONFIG_HOME/super-lazygit/config.toml`
3. `$HOME/.config/super-lazygit/config.toml`
4. built-in defaults

Try pointing the app at a directory that actually contains Git repositories. The discovery layer ignores `.git`, `node_modules`, and `target` by default.

## I expected a config file to be loaded

Config loading is active now. Common failure cases are:

- `SUPER_LAZYGIT_CONFIG` points at a file that does not exist
- the discovered config file is unreadable
- the TOML is invalid

All of those fail startup with the path included in the error message.

## My keybinding override does not take effect

Keybinding overrides only apply to routed command keys. They do not remap freeform character insertion or paste behavior in the workspace search box, input prompts, or commit box.

Other details that matter:

- overrides replace the default binding for that action; they do not add an alias on top of the default
- action names are canonicalized, so `enter_repo_mode` and `EnterRepoMode` target the same action
- single-character bindings are case-sensitive, so `p` and `P` are different keys
- `space` and a literal single space are treated as the same key

See [CONFIG.md](CONFIG.md) for the supported action IDs.

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

If you want a clean bootstrap path, remove the cache directory:

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

The repo-mode branch prompt also supports upstream assignment.

## Branch delete is more forceful than plain `git branch -d`

The delete-branch implementation uses `git branch -D` behind an explicit confirmation modal. That choice is deliberate: the softer `-d` path blocked confirmed-destructive flows in cases where upstream merge state prevented deletion even though the user had already confirmed the destructive action.

If you need the safer refusal semantics of `git branch -d`, use Git directly for now.

## I need deeper visibility into startup behavior

Diagnostics are enabled by default, and startup emits a stderr line when diagnostics logging is on. The test suite also covers the main workflows directly:

- workspace bootstrap and cache hydration
- watcher degradation and recovery
- repo summary refresh behavior
- commit, push, pull, stash, reflog, and branch lifecycle flows
- performance budgets for cold start, warm start, refresh, and detail loads
