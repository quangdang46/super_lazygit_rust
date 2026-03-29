# Keybindings

This document reflects the current reducer and TUI shell behavior in the repository. It is the source-of-truth contract for the interactive loop that is still being wired into the app binary.

## Global

| Key | Action |
| --- | --- |
| `?` | Open help modal |
| `Tab` | Move focus to the next pane |
| `Shift+Tab` | Move focus to the previous pane |
| `Esc` | Leave repo mode, or close the current modal |

## Workspace mode

| Key | Action |
| --- | --- |
| `j` / `Down` | Select next repository |
| `k` / `Up` | Select previous repository |
| `h` / `Left` | Focus the repo list pane |
| `l` / `Right` | Focus the preview pane |
| `/` | Focus workspace search |
| `f` | Cycle workspace filter: `all -> dirty -> ahead -> behind -> conflicts` |
| `s` | Cycle workspace sort: `attention -> name -> path -> activity` |
| `Enter` | Open the selected repository in repo mode |
| `r` | Refresh visible repositories |

### Workspace search overlay

| Key | Action |
| --- | --- |
| Any printable character | Append to the search query |
| `Space` | Insert a space |
| `Backspace` | Delete the previous character |
| `Enter` | Blur the search box and keep the query active |
| `Esc` | Cancel the search focus; if the query is non-empty it also clears the query |
| Paste | Insert pasted text into the query |

## Repo mode

### General

| Key | Action |
| --- | --- |
| `1` | Switch detail pane to `Status` |
| `2` | Switch detail pane to `Branches` |
| `3` | Switch detail pane to `Commits` |
| `4` | Switch detail pane to `Stash` scaffold |
| `5` | Switch detail pane to `Reflog` scaffold |
| `6` | Switch detail pane to `Worktrees` scaffold |
| `f` | Open fetch confirmation |
| `p` | Open pull confirmation |
| `P` | Push current branch immediately |
| `Esc` | Return to workspace mode |

### Working tree and staged panes

| Key | Action |
| --- | --- |
| `j` / `Down` | Select next status entry |
| `k` / `Up` | Select previous status entry |
| `Enter` in `Working tree` | Stage the selected file |
| `Enter` in `Staged changes` | Unstage the selected file |
| `c` in `Staged changes` | Open commit box |
| `A` | Open amend commit box |

### Status detail subview

| Key | Action |
| --- | --- |
| `j` | Move to the next selected hunk / scroll deeper into the diff |
| `k` | Move upward in the diff |

### Branches detail subview

| Key | Action |
| --- | --- |
| `j` / `Down` | Select next branch |
| `k` / `Up` | Select previous branch |
| `Enter` | Check out the selected branch |
| `c` | Open create-branch prompt |
| `R` | Open rename-branch prompt for the selected branch |
| `d` | Open delete-branch confirmation for the selected branch |
| `u` | Open set-upstream prompt for the selected branch |

### Commits detail subview

| Key | Action |
| --- | --- |
| `j` / `Down` | Select next commit |
| `k` / `Up` | Select previous commit |

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

### Commit box

| Key | Action |
| --- | --- |
| Any printable character | Append commit text |
| `Space` | Insert a space |
| `Backspace` | Delete the previous character |
| `Enter` | Submit the commit or amend action |
| `Esc` | Close the commit box without leaving repo mode |

## Notes

- `stash`, `reflog`, and `worktrees` are navigable subview targets today, but they are still scaffold surfaces rather than complete action flows.
- `push` is intentionally bound to uppercase `P` to avoid colliding with lowercase `p` for pull.
- Branch deletion is a confirmed destructive flow and uses the explicit confirmation overlay before the Git command is executed.
