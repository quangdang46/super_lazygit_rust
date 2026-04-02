# Config Reference

`super_lazygit_rust` loads configuration from disk at startup through `AppConfig::load()`.

## Load contract

Config discovery order is:

1. `SUPER_LAZYGIT_CONFIG`
2. `$XDG_CONFIG_HOME/super-lazygit/config.toml`
3. `$HOME/.config/super-lazygit/config.toml`
4. built-in defaults when no file exists

Behavior details:

- if `SUPER_LAZYGIT_CONFIG` is set, that path is mandatory; startup fails if the file does not exist
- if the selected config file cannot be read, startup fails with the path in the error
- if TOML parsing fails, startup fails with the path in the error
- after config loads, workspace-root resolution is: CLI `--workspace`, then `workspace.roots[0]`, then the current working directory

## Schema

### `workspace`

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `roots` | `Vec<PathBuf>` | `[]` | Preferred workspace roots used when `--workspace` is omitted |
| `ignores` | `Vec<String>` | `[".git", "node_modules", "target"]` | Discovery ignore list |

### `os`

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `open` | `String` | `""` | Full shell template for opening a file target; supports `{{filename}}` and `{{.filename}}` |
| `open_link` | `String` | `""` | Full shell template for opening a browser/link target; supports `{{link}}` and `{{.link}}` |
| `copy_to_clipboard_cmd` | `String` | `""` | Full shell template for writing to the clipboard; supports `{{text}}` and `{{.text}}` |
| `read_from_clipboard_cmd` | `String` | `""` | Shell command used when reading from the clipboard |
| `shell_functions_file` | `String` | `""` | Optional shell startup file sourced before generated shell commands run |

OS command behavior:

- When `open`, `open_link`, or `copy_to_clipboard_cmd` are empty, the runtime keeps the built-in cross-platform fallbacks.
- When `shell_functions_file` is set, generated shell commands source that file before running the main command body.
- Clipboard copy, browser-open, and default-app-open actions all route through this layer.

### `editor`

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `command` | `String` | `""` | Legacy command surface; only used when template fields are empty |
| `args` | `Vec<String>` | `[]` | Legacy argument list; each value is shell-quoted and `{{filename}}` or `{{dir}}` is appended automatically |
| `edit_preset` | `String` | `""` | Optional preset name such as `vim`, `nvim`, `vscode`, `helix`, `sublime`, `zed`, or `nvim-remote` |
| `edit` | `String` | `""` | Full shell template for opening a file |
| `edit_at_line` | `String` | `""` | Full shell template for opening a file at a line |
| `edit_at_line_and_wait` | `String` | `""` | Full shell template for opening a file at a line and waiting for exit |
| `open_dir_in_editor` | `String` | `""` | Full shell template for opening a directory target |
| `edit_in_terminal` | `Option<bool>` | `None` | Overrides whether the TUI suspends while the editor command runs |

Editor launch behavior:

- Runtime resolves editor commands in this order: explicit template field, legacy `command` plus `args`, preset selected by `edit_preset`, guessed preset from `git config core.editor`, `GIT_EDITOR`, `VISUAL`, `EDITOR`, then fallback `vim`.
- Repo mode passes the selected file path to `edit`, `edit_at_line`, or `edit_at_line_and_wait`.
- Workspace and worktree mode pass the selected repository root to `open_dir_in_editor`.
- The runtime sets the editor process current working directory to the selected repository root.
- All resolved editor commands are launched through the active shell: `sh -lc` on Unix and `cmd /C` on Windows.
- Template placeholders are substituted before launch: `{{filename}}` and `{{.filename}}` for file targets, `{{dir}}` and `{{.dir}}` for directory targets, and `{{line}}` and `{{.line}}` for line-aware commands.
- Built-in terminal presets such as `vim`, `nvim`, `helix`, `nano`, and `micro` suspend the TUI by default.
- Built-in GUI presets such as `vscode`, `sublime`, `bbedit`, `xcode`, `zed`, and `acme` do not suspend by default.
- `edit_in_terminal = true` forces suspend even for GUI presets. `edit_in_terminal = false` keeps the TUI live even for terminal presets.

Legacy `command` plus `args` compatibility:

- When any template field is set, that template wins for its command kind.
- When templates are empty but `command` or `args` are configured, the legacy command path is used.
- Legacy command mode shell-quotes the binary, shell-quotes each arg, and appends `{{filename}}` or `{{dir}}` automatically.

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
| `overrides` | `Vec<KeybindingOverride>` | `[]` | Replaces built-in routed command bindings for matching actions |

Each override has:

- `action: String`
- `keys: Vec<String>`

Override behavior:

- overrides replace the built-in keys for a routed command; they do not add extra aliases on top of the defaults
- action IDs are canonicalized by stripping non-alphanumeric characters and lowercasing, so `enter_repo_mode`, `EnterRepoMode`, and `enterRepoMode` all target the same action
- named keys are lowercased during matching, so `Tab` and `tab` are equivalent
- `space` and a literal single space are equivalent
- single-character bindings are case-sensitive, so `p` and `P` are different keys
- freeform text entry is not remapped; printable text and paste handling in workspace search, input prompts, and the commit box still insert literal text

Supported routed action IDs:

- Global: `open_help`, `next_focus`, `previous_focus`, `leave_repo_mode`
- Modal and prompt overlays: `confirm_pending_operation`, `close_top_modal`, `submit_prompt_input`, `backspace_prompt_input`
- Workspace: `focus_workspace_search`, `select_next_repo`, `select_previous_repo`, `focus_workspace_preview`, `focus_workspace_list`, `cycle_workspace_filter`, `cycle_workspace_sort`, `enter_repo_mode`, `open_in_editor`, `refresh_visible_repos`, `cancel_workspace_search`, `blur_workspace_search`, `backspace_workspace_search`
- Repo navigation: `focus_repo_left`, `focus_repo_right`, `switch_repo_subview_status`, `switch_repo_subview_branches`, `switch_repo_subview_commits`, `switch_repo_subview_compare`, `switch_repo_subview_rebase`, `switch_repo_subview_stash`, `switch_repo_subview_reflog`, `switch_repo_subview_worktrees`, `refresh_selected_repo`, `fetch_selected_repo`, `pull_current_branch`, `push_current_branch`
- Repo status panes: `select_next_status_entry`, `select_previous_status_entry`, `stage_selected_file`, `unstage_selected_file`, `open_commit_box`, `open_amend_commit_box`, `discard_selected_file`, `open_in_editor`
- Status detail: `select_next_diff_line`, `select_previous_diff_line`, `select_next_diff_hunk`, `select_previous_diff_hunk`, `toggle_diff_line_anchor`, `scroll_repo_detail_down`, `scroll_repo_detail_up`, `apply_selected_hunk`, `apply_selected_lines`, `open_in_editor`, `nuke_working_tree`
- Branches detail: `select_next_branch`, `select_previous_branch`, `open_selected_branch_commits`, `checkout_selected_branch`, `checkout_previous_branch`, `open_checkout_branch_prompt`, `open_create_branch_prompt`, `open_rename_branch_prompt`, `delete_selected_branch`, `open_branch_upstream_options`, `copy_selected_branch_name`, `merge_selected_branch_into_current`, `rebase_current_branch_onto_selected_branch`, `toggle_comparison_selection`, `clear_comparison`
- Remote branches detail: `copy_selected_remote_branch_name`, `set_current_branch_upstream_to_selected_remote_branch`, `merge_selected_remote_branch_into_current`, `rebase_current_branch_onto_selected_remote_branch`
- Commits detail: `select_next_commit`, `select_previous_commit`, `copy_selected_commit_for_cherry_pick`, `cherry_pick_copied_commit`, `clear_copied_commit_selection`, `copy_selected_commit_hash`, `open_commit_copy_options`, `open_selected_commit_in_browser`, `open_commit_log_options`, `start_interactive_rebase`, `amend_selected_commit`, `open_commit_amend_attribute_options`, `open_commit_fixup_options`, `create_fixup_commit`, `fixup_selected_commit`, `set_fixup_message_for_selected_commit`, `reword_selected_commit`, `reword_selected_commit_with_editor`, `revert_selected_commit`, `soft_reset_to_selected_commit`, `mixed_reset_to_selected_commit`, `hard_reset_to_selected_commit`, `toggle_comparison_selection`, `clear_comparison`
- Rebase detail: `continue_rebase`, `skip_rebase`, `abort_rebase`, `scroll_repo_detail_down`, `scroll_repo_detail_up`
- Stash detail: `select_next_stash`, `select_previous_stash`, `apply_selected_stash`, `open_create_branch_from_stash_prompt`, `open_rename_stash_prompt`, `pop_selected_stash`, `drop_selected_stash`
- Reflog detail: `select_next_reflog`, `select_previous_reflog`, `open_selected_reflog_commits`, `checkout_selected_commit`, `open_create_branch_from_commit_prompt`, `cherry_pick_selected_commit`, `soft_reset_to_selected_commit`, `mixed_reset_to_selected_commit`, `hard_reset_to_selected_commit`, `restore_selected_reflog_entry`
- Worktrees detail: `select_next_worktree`, `select_previous_worktree`, `switch_to_selected_worktree`, `create_worktree`, `open_in_editor`, `remove_selected_worktree`
- Commit box: `cancel_commit_box`, `submit_commit_box`, `backspace_commit_input`

### `diagnostics`

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `enabled` | `bool` | `true` | Enables diagnostics collection |
| `log_samples` | `bool` | `true` | Emits a startup diagnostics line to stderr |
| `slow_render_threshold_ms` | `u64` | `16` | Threshold for slow-render sampling |
| `watcher_burst_threshold` | `usize` | `8` | Threshold for watcher burst diagnostics |

## Example

```toml
[workspace]
roots = ["/path/to/workspace"]
ignores = [".git", "node_modules", "target"]

[os]
open = "code --reuse-window -- {{filename}}"
open_link = "open {{link}}"
copy_to_clipboard_cmd = "printf '%s' {{text}} | pbcopy"
read_from_clipboard_cmd = "pbpaste"
shell_functions_file = "~/.config/lazygit/shell-functions.sh"

[editor]
edit_preset = "vscode"
edit = "code --reuse-window -- {{filename}}"
edit_at_line = "code --reuse-window --goto -- {{filename}}:{{line}}"
edit_at_line_and_wait = "code --reuse-window --goto --wait -- {{filename}}:{{line}}"
open_dir_in_editor = "code -- {{dir}}"
edit_in_terminal = false

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
action = "enter_repo_mode"
keys = ["o"]

[[keybindings.overrides]]
action = "PushCurrentBranch"
keys = ["g"]

[diagnostics]
enabled = true
log_samples = true
slow_render_threshold_ms = 16
watcher_burst_threshold = 8
```

## Non-config settings

Some runtime settings still live in reducer state rather than `AppConfig`, for example destructive-operation confirmation state, help-footer visibility, and selected theme/keymap names inside `SettingsSnapshot`. Those are internal runtime surfaces today, not user-facing config fields.
