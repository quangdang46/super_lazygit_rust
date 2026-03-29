# Config Reference

`super_lazygit_rust` already has a real config schema in [`crates/config/src/lib.rs`](../crates/config/src/lib.rs), but the application does not load a config file from disk yet. The binary currently constructs `AppConfig::default()` directly.

This page documents the schema and defaults so operators know what is already modeled and what is still pending on the app side.

## Current behavior

- `--workspace <path>` is the only runtime input the binary reads today
- if `--workspace` is omitted, the app falls back to the current directory
- config-file discovery and parsing are not wired yet
- keybinding overrides exist in the schema but are not applied yet

## Schema

### `workspace`

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `roots` | `Vec<PathBuf>` | `[]` | Reserved for preferred workspace roots |
| `ignores` | `Vec<String>` | `[".git", "node_modules", "target"]` | Discovery ignore list |

### `editor`

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `command` | `String` | `"vim"` | Editor command placeholder surface |
| `args` | `Vec<String>` | `[]` | Extra editor arguments |

### `theme`

| Field | Type | Default |
| --- | --- | --- |
| `preset` | `ThemePreset` | `default_dark` |
| `colors.background` | `String` | `#111318` |
| `colors.foreground` | `String` | `#d8dee9` |
| `colors.accent` | `String` | `#88c0d0` |
| `colors.success` | `String` | `#a3be8c` |
| `colors.warning` | `String` | `#ebcb8b` |
| `colors.danger` | `String` | `#bf616a` |

### `keybindings`

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `overrides` | `Vec<KeybindingOverride>` | `[]` | Schema exists, runtime application is not implemented yet |

Each override has:

- `action: String`
- `keys: Vec<String>`

### `diagnostics`

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `enabled` | `bool` | `true` | Enables startup diagnostics collection |
| `log_samples` | `bool` | `true` | Emits a startup diagnostics line to stderr |
| `slow_render_threshold_ms` | `u64` | `16` | Threshold for slow-render sampling |
| `watcher_burst_threshold` | `usize` | `8` | Threshold for watcher burst diagnostics |

## Example schema snapshot

This is a schema example, not a currently loaded file format contract:

```toml
[workspace]
roots = ["/path/to/workspace"]
ignores = [".git", "node_modules", "target"]

[editor]
command = "vim"
args = []

[theme]
preset = "default_dark"

[theme.colors]
background = "#111318"
foreground = "#d8dee9"
accent = "#88c0d0"
success = "#a3be8c"
warning = "#ebcb8b"
danger = "#bf616a"

[[keybindings.overrides]]
action = "EnterRepoMode"
keys = ["enter"]

[diagnostics]
enabled = true
log_samples = true
slow_render_threshold_ms = 16
watcher_burst_threshold = 8
```

## Non-config settings

Some runtime settings currently live in reducer state rather than in `AppConfig`, for example:

- destructive-operation confirmation behavior
- help-footer visibility
- selected keymap and theme names inside `SettingsSnapshot`

Those settings are internal state surfaces for now, not part of the user-facing config schema.
