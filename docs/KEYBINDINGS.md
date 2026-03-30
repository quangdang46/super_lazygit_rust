# Keybindings

This document reflects the current interactive terminal behavior shipped by the TUI router and runtime.

## Global

| Key | Action |
| --- | --- |
| `?` | Open the help modal |
| `Tab` | Move focus to the next pane |
| `Shift+Tab` | Move focus to the previous pane |
| `Esc` | Leave repo mode, or close the active modal/overlay |

## Workspace mode

| Key | Action |
| --- | --- |
| `j` / `Down` | Select the next repository |
| `k` / `Up` | Select the previous repository |
| `h` / `Left` | Focus the repo list pane |
| `l` / `Right` | Focus the preview pane |
| `/` | Focus workspace search |
| `f` | Cycle workspace filter: `all -> dirty -> ahead -> behind -> conflicts` |
| `s` | Cycle workspace sort: `attention -> name -> path -> activity` |
| `Enter` | Open the selected repository in repo mode |
| `e` | Open the selected repository root in the configured editor |
| `r` | Refresh visible repositories |

### Workspace search overlay

| Key | Action |
| --- | --- |
| Any printable character | Append to the search query |
| `Space` | Insert a space |
| `Backspace` | Delete the previous character |
| `Enter` | Blur the search box and keep the query active |
| `Esc` | Cancel search focus; if the query is non-empty it also clears the query |
| Paste | Insert pasted text into the query |

## Repo mode

### General

| Key | Action |
| --- | --- |
| `h` / `Left` | Move focus one pane left |
| `l` / `Right` | Move focus one pane right |
| `1` | Switch detail pane to `Status` |
| `2` | Switch detail pane to `Branches` |
| `3` | Switch detail pane to `Commits` |
| `4` | Switch detail pane to `Compare` |
| `5` | Switch detail pane to `Rebase` |
| `6` | Switch detail pane to `Stash` |
| `7` | Switch detail pane to `Reflog` |
| `8` | Switch detail pane to `Worktrees` |
| `r` | Refresh the selected repository |
| `f` | Open fetch confirmation |
| `p` | Open pull confirmation |
| `P` | Open push confirmation |
| `Esc` | Return to workspace mode |

### Working tree and staged panes

| Key | Action |
| --- | --- |
| `j` / `Down` | Select the next status entry |
| `k` / `Up` | Select the previous status entry |
| `Enter` or `Space` in `Working tree` | Stage the selected file |
| `Enter` or `Space` in `Staged changes` | Unstage the selected file |
| `c` in `Staged changes` | Open the commit box |
| `w` in `Staged changes` | Open the no-verify commit box |
| `C` in `Working tree` or `Staged changes` | Commit staged changes using the configured Git editor |
| `s` in `Working tree` or `Staged changes` | Open the tracked-changes stash prompt |
| `S` in `Working tree` or `Staged changes` | Open the stash-options menu, then choose tracked, keep-index, include-untracked, staged, or unstaged stash creation |
| `A` in `Staged changes` | Open the amend commit box |
| `D` | Open discard confirmation for the selected file |
| `e` | Open the selected file in the configured editor |

### Status detail subview

| Key | Action |
| --- | --- |
| `J` | Select the next diff line |
| `K` | Select the previous diff line |
| `j` | Select the next diff hunk |
| `k` | Select the previous diff hunk |
| `v` | Toggle the diff-line anchor |
| `Down` | Scroll the detail pane down |
| `Up` | Scroll the detail pane up |
| `Enter` or `Space` | Stage or unstage the selected hunk, depending on diff orientation |
| `L` | Stage or unstage the selected line range, depending on diff orientation |
| `D` | Open discard confirmation for the selected file |
| `e` | Open the selected diff file in the configured editor |
| `X` | Open destructive confirmation for nuking the working tree |

### Branches detail subview

| Key | Action |
| --- | --- |
| `j` / `Down` | Select the next branch |
| `k` / `Up` | Select the previous branch |
| `Enter` | Check out the selected branch |
| `c` | Open create-branch prompt |
| `R` | Open rename-branch prompt |
| `d` | Open delete-branch confirmation |
| `u` | Open set-upstream prompt |
| `v` | Toggle comparison selection |
| `x` | Clear comparison when one is active |

### Commits detail subview

| Key | Action |
| --- | --- |
| `j` / `Down` | Select the next commit |
| `k` / `Up` | Select the previous commit |
| `i` | Start interactive rebase from the selected commit |
| `A` | Open amend-selected-commit confirmation |
| `F` | Open fixup-selected-commit confirmation |
| `R` | Reword the selected commit using the configured Git editor |
| `C` | Open cherry-pick confirmation |
| `V` | Open revert confirmation |
| `S` | Open soft-reset confirmation |
| `M` | Open mixed-reset confirmation |
| `H` | Open hard-reset confirmation |
| `v` | Toggle comparison selection |
| `x` | Clear comparison when one is active |

### Compare detail subview

| Key | Action |
| --- | --- |
| `j` / `Down` | Scroll down |
| `k` / `Up` | Scroll up |
| `x` | Clear the active comparison |

### Rebase detail subview

| Key | Action |
| --- | --- |
| `j` / `Down` | Scroll down |
| `k` / `Up` | Scroll up |
| `c` | Continue rebase |
| `s` | Skip the current rebase step |
| `A` | Abort rebase |

### Stash detail subview

| Key | Action |
| --- | --- |
| `j` / `Down` | Select the next stash entry |
| `k` / `Up` | Select the previous stash entry |
| `Enter` | Apply the selected stash |
| `r` | Open rename-stash prompt |
| `g` | Open pop-stash confirmation |
| `d` | Open drop-stash confirmation |

### Reflog detail subview

| Key | Action |
| --- | --- |
| `j` / `Down` | Select the next reflog entry |
| `k` / `Up` | Select the previous reflog entry |
| `u` | Open restore confirmation for the selected reflog entry |

### Worktrees detail subview

| Key | Action |
| --- | --- |
| `j` / `Down` | Select the next worktree |
| `k` / `Up` | Select the previous worktree |
| `c` | Open create-worktree prompt |
| `d` | Open remove-worktree confirmation |

## Overlays

### Confirmation modal

| Key | Action |
| --- | --- |
| `Enter` / `y` | Confirm the action |
| `Esc` / `q` / `n` | Cancel the action |

### Input prompt modal

| Key | Action |
| --- | --- |
| Any printable character | Append text |
| `Space` | Insert a space |
| `Backspace` | Delete the previous character |
| `Enter` | Submit the prompt |
| `Esc` / `q` | Cancel the prompt |
| Paste | Insert pasted text |

### Menu modal

| Key | Action |
| --- | --- |
| `j` / `Down` | Select the next menu item |
| `k` / `Up` | Select the previous menu item |
| `Enter` | Confirm the selected menu item |
| `Esc` / `q` | Cancel the menu |

### Commit box

| Key | Action |
| --- | --- |
| Any printable character | Append commit text |
| `Space` | Insert a space |
| `Backspace` | Delete the previous character |
| `Enter` | Submit the commit or amend action |
| `Esc` | Close the commit box without leaving repo mode |

## Keybinding overrides

- Routed command bindings can be replaced from config; see [CONFIG.md](CONFIG.md).
- Override action IDs accept both stable snake_case names such as `enter_repo_mode` and legacy enum-style names such as `EnterRepoMode`.
- Single-character overrides are case-sensitive, so rebinding `push_current_branch` must use `P` or another exact single-character key if you want uppercase behavior.
- Text insertion and paste behavior in the workspace search box, input prompts, and commit box are intentionally not remapped.
