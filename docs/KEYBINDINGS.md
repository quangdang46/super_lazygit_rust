# Keybindings

This document reflects the current interactive terminal behavior shipped by the TUI router and runtime.

## Global

| Key | Action |
| --- | --- |
| `?` | Open the help modal |
| `Tab` | Move focus to the next pane |
| `Shift+Tab` | Move focus to the previous pane |
| `Esc` | Leave repo mode, close the active modal/overlay, or return from status detail to the last files pane |

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
| `b` | Switch detail pane to `Submodules` |
| `m` in most `Repo detail` contexts | Switch detail pane to `Remotes` |
| `9` | Switch detail pane to `Remote Branches` |
| `t` | Switch detail pane to `Tags` |
| `0` in `Repo detail` | Return focus to the last active main pane (`Working tree` or `Staged changes`) |
| `,` / `PageUp` on list-focused repo surfaces | Move one page up in `Working tree`, `Staged changes`, or list-style detail subviews |
| `.` / `PageDown` on list-focused repo surfaces | Move one page down in `Working tree`, `Staged changes`, or list-style detail subviews |
| `<` / `Home` on list-focused repo surfaces | Jump to the first entry in the current list |
| `>` / `End` on list-focused repo surfaces | Jump to the last entry in the current list |
| `[` / `]` | Move to the previous or next repository detail subview tab |
| `/` in filterable `Repo detail` subviews | Focus the panel-local filter (`Branches`, `Remotes`, `Remote Branches`, `Tags`, `Commits`, `Stash`, `Reflog`, `Worktrees`, or `Submodules`) |
| `w` in `Repo detail` | Switch the current detail subview to `Worktrees` |
| `r` | Refresh the selected repository |
| `R` | Run a deeper repository refresh: refresh the selected repo, reload detail, and rescan the workspace |
| `Ctrl+R` | Open the recent-repositories menu |
| `:` | Open the shell-command prompt for the current repository root |
| `@` | Open the command-log menu for recent session messages |
| `+` | Cycle to the next repo screen mode: `normal -> larger main pane -> fullscreen focused pane` |
| `_` | Cycle to the previous repo screen mode |
| `Ctrl+Z` | Suspend the TUI and resume it with your shell's `fg` |
| `Ctrl+S` in filterable `Repo detail` subviews | Open the filter-options menu for the active detail panel |
| `W` / `Ctrl+E` in `Status`, `Branches`, `Commits`, or `Compare` detail | Open the diffing-options menu for the current diff/comparison context |
| `Ctrl+W` in `Status`, `Commits`, or `Compare` detail | Toggle whitespace visibility in the current diff |
| `}` / `{` in `Status`, `Commits`, or `Compare` detail | Increase or decrease diff context lines |
| `)` / `(` in `Status`, `Commits`, or `Compare` detail | Increase or decrease rename similarity threshold |
| `m` in `Commits` or `Rebase` detail | Open the merge/rebase options menu for the shipped flows in the current context |
| `Ctrl+P` in `Status detail` | Open the patch-options menu for shipped hunk/line patch flows |
| `f` | Open fetch confirmation |
| `p` | Open pull confirmation |
| `P` | Open push confirmation |
| `Esc` | Return to workspace mode from the files panes; in `Status detail`, return to the last focused files pane instead |

### Repo detail contract

When focus is in `Repo detail`, the shared lazygit-style contract is:

| Key | Action |
| --- | --- |
| `Enter` | Run the contextual primary action for the current detail subview |
| `Space` | Run the contextual checkout/apply action in detail subviews that expose one |
| `0` | Return to the last active main pane without changing the selected detail subview |
| `,` / `PageUp` | Move one page up on list-style detail surfaces |
| `.` / `PageDown` | Move one page down on list-style detail surfaces |
| `<` / `Home` | Jump to the first entry on list-style detail surfaces |
| `>` / `End` | Jump to the last entry on list-style detail surfaces |
| `[` / `]` | Move to the previous or next detail tab |
| `/` | Focus the current subview filter when that subview supports filtering |
| `w` | Jump directly to the `Worktrees` subview |
| `b` | Jump directly to the `Submodules` subview |

### Working tree and staged panes

| Key | Action |
| --- | --- |
| `j` / `Down` | Select the next status entry |
| `k` / `Up` | Select the previous status entry |
| `,` / `PageUp` | Move one page up in the current status list |
| `.` / `PageDown` | Move one page down in the current status list |
| `<` / `Home` | Jump to the first status entry |
| `>` / `End` | Jump to the last status entry |
| `Enter` or `Space` in `Working tree` | Stage the selected file |
| `Enter` or `Space` in `Staged changes` | Unstage the selected file |
| `c` in `Staged changes` | Open the commit box |
| `w` in `Staged changes` | Open the no-verify commit box |
| `C` in `Working tree` or `Staged changes` | Commit staged changes using the configured Git editor |
| `s` in `Working tree` or `Staged changes` | Open the tracked-changes stash prompt |
| `S` in `Working tree` or `Staged changes` | Open the stash-options menu, then choose tracked, keep-index, include-untracked, staged, or unstaged stash creation |
| `a` in `Working tree` or `Staged changes` | Open the all-branches commit graph |
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
| `Ctrl+W` | Toggle whitespace visibility in the current diff |
| `}` / `{` | Increase or decrease diff context lines |
| `)` / `(` | Increase or decrease rename similarity threshold |
| `W` or `Ctrl+E` | Open the diffing-options menu for the current status diff |
| `Ctrl+P` | Open the patch-options menu for the currently selected diff |
| `D` | Open discard confirmation for the selected file |
| `e` | Open the selected diff file in the configured editor |
| `a` | Open the all-branches commit graph, newest first |
| `A` | Open the all-branches commit graph, oldest first |
| `X` | Open destructive confirmation for nuking the working tree |
| `Esc` / `0` | Return to the last focused files pane |

### Branches detail subview

| Key | Action |
| --- | --- |
| `j` / `Down` | Select the next branch |
| `k` / `Up` | Select the previous branch |
| `,` / `PageUp` | Move one page up in the filtered branch list |
| `.` / `PageDown` | Move one page down in the filtered branch list |
| `<` / `Home` | Jump to the first visible branch |
| `>` / `End` | Jump to the last visible branch |
| `[` / `]` | Move to the previous or next detail tab |
| `Enter` | Open the selected branch's commit history |
| `Space` | Check out the selected branch |
| `-` | Check out the previous branch |
| `c` | Open checkout-by-name prompt (`-` switches to the previous branch) |
| `n` | Open create-branch prompt |
| `R` | Open rename-branch prompt |
| `d` | Open delete-branch confirmation |
| `u` | Open set-upstream prompt |
| `v` | Toggle comparison selection |
| `x` | Clear comparison when one is active |
| `Ctrl+S` | Open the filter-options menu for the branch list |
| `W` or `Ctrl+E` | Open the diffing-options menu for branch comparison flows and diff settings |
| `w` | Switch to the worktrees subview |

### Remotes detail subview

| Key | Action |
| --- | --- |
| `j` / `Down` | Select the next remote |
| `k` / `Up` | Select the previous remote |
| `,` / `PageUp` | Move one page up in the filtered remote list |
| `.` / `PageDown` | Move one page down in the filtered remote list |
| `<` / `Home` | Jump to the first visible remote |
| `>` / `End` | Jump to the last visible remote |
| `[` / `]` | Move to the previous or next detail tab |
| `Enter` | Open the selected remote's branch list in `Remote Branches` |
| `n` | Open the add-remote prompt (`<name> <url>`) |
| `e` | Open the edit-remote prompt for the selected remote |
| `d` | Open remove-remote confirmation |
| `f` | Open fetch-remote confirmation for the selected remote |
| `w` | Switch to the worktrees subview |

### Remote branches detail subview

| Key | Action |
| --- | --- |
| `j` / `Down` | Select the next remote branch |
| `k` / `Up` | Select the previous remote branch |
| `,` / `PageUp` | Move one page up in the filtered remote-branch list |
| `.` / `PageDown` | Move one page down in the filtered remote-branch list |
| `<` / `Home` | Jump to the first visible remote branch |
| `>` / `End` | Jump to the last visible remote branch |
| `[` / `]` | Move to the previous or next detail tab |
| `Enter` | Open the selected remote branch's commit history |
| `Space` | Check out the selected remote branch as a local tracking branch |
| `n` | Open create-local-branch-from-remote prompt |
| `d` | Open delete-remote-branch confirmation |
| `w` | Switch to the worktrees subview |

### Tags detail subview

| Key | Action |
| --- | --- |
| `j` / `Down` | Select the next tag |
| `k` / `Up` | Select the previous tag |
| `,` / `PageUp` | Move one page up in the tag list |
| `.` / `PageDown` | Move one page down in the tag list |
| `<` / `Home` | Jump to the first visible tag |
| `>` / `End` | Jump to the last visible tag |
| `[` / `]` | Move to the previous or next detail tab |
| `Enter` | Open the selected tag's commit history |
| `Space` | Check out the selected tag in detached-HEAD mode |
| `n` | Open create-tag prompt |
| `d` | Open delete-tag confirmation |
| `P` | Open push-tag confirmation |
| `S` | Open soft-reset-to-tag confirmation |
| `M` | Open mixed-reset-to-tag confirmation |
| `H` | Open hard-reset-to-tag confirmation |
| `w` | Switch to the worktrees subview |

### Commits detail subview

| Key | Action |
| --- | --- |
| `j` / `Down` | Select the next commit |
| `k` / `Up` | Select the previous commit |
| `,` / `PageUp` | Move one page up in the filtered commit list or commit-files list |
| `.` / `PageDown` | Move one page down in the filtered commit list or commit-files list |
| `<` / `Home` | Jump to the first visible commit or commit file |
| `>` / `End` | Jump to the last visible commit or commit file |
| `[` / `]` | Move to the previous or next detail tab |
| `Enter` | Open the selected commit's changed-files view |
| `Space` | Check out the selected commit in detached-HEAD mode |
| `a` | Switch to the all-branches commit graph, newest first |
| `A` | Switch to the all-branches commit graph, oldest first |
| `n` | Open create-branch-from-commit prompt |
| `T` | Open create-tag-from-commit prompt |
| `i` | Start interactive rebase from the selected commit |
| `A` | Open amend-selected-commit confirmation |
| `f` | Create a fixup commit for the selected commit from the currently staged changes |
| `F` | Open fixup-selected-commit confirmation |
| `g` | Open apply-fixups confirmation for the selected commit |
| `s` | Open squash-selected-commit confirmation |
| `d` | Open drop-selected-commit confirmation |
| `m` | Open the merge/rebase options menu for the selected commit or active rebase |
| `R` | Reword the selected commit using the configured Git editor |
| `C` | Open cherry-pick confirmation |
| `V` | Open revert confirmation |
| `S` | Open soft-reset confirmation |
| `M` | Open mixed-reset confirmation |
| `H` | Open hard-reset confirmation |
| `v` | Toggle comparison selection |
| `x` | Clear comparison when one is active |
| `Ctrl+W` | Toggle whitespace visibility in the current diff |
| `}` / `{` | Increase or decrease diff context lines |
| `)` / `(` | Increase or decrease rename similarity threshold |
| `Ctrl+S` | Open the filter-options menu for the commit history or commit-files list |
| `W` or `Ctrl+E` | Open the diffing-options menu for commit comparison flows and diff settings |
| `/` | Focus the commit-history filter |
| `w` | Switch to the worktrees subview |
| `0` | Return focus to the last active main pane |

### Commit files detail mode

| Key | Action |
| --- | --- |
| `j` / `Down` | Select the next changed file from the active commit |
| `k` / `Up` | Select the previous changed file from the active commit |
| `,` / `PageUp` | Move one page up in the changed-files list |
| `.` / `PageDown` | Move one page down in the changed-files list |
| `<` / `Home` | Jump to the first visible changed file |
| `>` / `End` | Jump to the last visible changed file |
| `[` / `]` | Move to the previous or next detail tab |
| `Enter` | Return to commit history for the same selected commit |
| `Space` | Check out the selected file from the selected commit |
| `e` | Open the selected file in the configured editor |
| `/` | Focus the changed-files filter |
| `0` | Return focus to the last active main pane |
| `w` | Switch to the worktrees subview |

### Compare detail subview

| Key | Action |
| --- | --- |
| `j` / `Down` | Scroll down |
| `k` / `Up` | Scroll up |
| `Ctrl+W` | Toggle whitespace visibility in the active comparison |
| `}` / `{` | Increase or decrease diff context lines |
| `)` / `(` | Increase or decrease rename similarity threshold |
| `W` or `Ctrl+E` | Open the diffing-options menu for the active comparison |
| `x` | Clear the active comparison |

### Rebase detail subview

| Key | Action |
| --- | --- |
| `j` / `Down` | Scroll down |
| `k` / `Up` | Scroll up |
| `c` | Continue rebase |
| `s` | Skip the current rebase step |
| `A` | Abort rebase |
| `m` | Open the merge/rebase options menu for the active rebase |

### Stash detail subview

| Key | Action |
| --- | --- |
| `j` / `Down` | Select the next stash entry |
| `k` / `Up` | Select the previous stash entry |
| `,` / `PageUp` | Move one page up in the filtered stash list or stash-files list |
| `.` / `PageDown` | Move one page down in the filtered stash list or stash-files list |
| `<` / `Home` | Jump to the first visible stash or stash file |
| `>` / `End` | Jump to the last visible stash or stash file |
| `[` / `]` | Move to the previous or next detail tab |
| `Enter` | Open the selected stash's changed-files view |
| `Space` | Apply the selected stash |
| `n` | Open create-branch-from-stash prompt |
| `r` | Open rename-stash prompt |
| `g` | Open pop-stash confirmation |
| `d` | Open drop-stash confirmation |
| `w` | Switch to the worktrees subview |

### Stash files detail mode

| Key | Action |
| --- | --- |
| `j` / `Down` | Select the next changed file from the selected stash |
| `k` / `Up` | Select the previous changed file from the selected stash |
| `,` / `PageUp` | Move one page up in the stash-files list |
| `.` / `PageDown` | Move one page down in the stash-files list |
| `<` / `Home` | Jump to the first stash file |
| `>` / `End` | Jump to the last stash file |
| `[` / `]` | Move to the previous or next detail tab |
| `Enter` | Return to the stash list for the same selected stash |
| `0` | Return focus to the last active main pane |
| `w` | Switch to the worktrees subview |

### Reflog detail subview

| Key | Action |
| --- | --- |
| `j` / `Down` | Select the next reflog entry |
| `k` / `Up` | Select the previous reflog entry |
| `,` / `PageUp` | Move one page up in the filtered reflog list |
| `.` / `PageDown` | Move one page down in the filtered reflog list |
| `<` / `Home` | Jump to the first visible reflog entry |
| `>` / `End` | Jump to the last visible reflog entry |
| `[` / `]` | Move to the previous or next detail tab |
| `Enter` | Open the selected reflog target in the all-branches commit history view |
| `Space` | Check out the selected reflog target in detached HEAD |
| `n` | Open create-branch-from-commit prompt for the selected reflog target |
| `T` | Open create-tag-from-commit prompt for the selected reflog target |
| `C` | Open cherry-pick confirmation for the selected reflog target |
| `S` | Open soft-reset confirmation using the selected reflog selector |
| `M` | Open mixed-reset confirmation using the selected reflog selector |
| `H` | Open hard-reset confirmation using the selected reflog selector |
| `u` | Open restore confirmation for the selected reflog entry |

### Worktrees detail subview

| Key | Action |
| --- | --- |
| `j` / `Down` | Select the next worktree |
| `k` / `Up` | Select the previous worktree |
| `,` / `PageUp` | Move one page up in the filtered worktree list |
| `.` / `PageDown` | Move one page down in the filtered worktree list |
| `<` / `Home` | Jump to the first visible worktree |
| `>` / `End` | Jump to the last visible worktree |
| `[` / `]` | Move to the previous or next detail tab |
| `Enter` / `Space` | Switch to the selected worktree |
| `n` | Open create-worktree prompt |
| `c` | Open create-worktree prompt (legacy alias) |
| `o` | Open the selected worktree in the configured editor |
| `d` | Open remove-worktree confirmation |
| `/` | Focus the worktree filter |
| `0` | Return focus to the last active main pane |

### Submodules detail subview

| Key | Action |
| --- | --- |
| `j` / `Down` | Select the next submodule |
| `k` / `Up` | Select the previous submodule |
| `,` / `PageUp` | Move one page up in the filtered submodule list |
| `.` / `PageDown` | Move one page down in the filtered submodule list |
| `<` / `Home` | Jump to the first visible submodule |
| `>` / `End` | Jump to the last visible submodule |
| `[` / `]` | Move to the previous or next detail tab |
| `Enter` / `Space` | Enter the selected submodule as a nested repo |
| `n` | Open add-submodule prompt (`<path> <url>`) |
| `e` | Open edit-submodule-URL prompt |
| `i` | Initialize the selected submodule |
| `u` | Update the selected submodule |
| `o` | Open the selected submodule path in the configured editor |
| `d` | Open remove-submodule confirmation |
| `/` | Focus the submodule filter |
| `0` | Return focus to the last active main pane |
| `Esc` | Return to the parent repo when currently inside a nested submodule repo; otherwise leave repo mode |

### Repo detail filter

| Key | Action |
| --- | --- |
| Any printable character | Append text to the active panel-local filter |
| `Space` | Insert a space |
| `Backspace` | Delete the previous character |
| `Enter` | Blur the filter and keep the query active |
| `Esc` | Exit the filter; if the query is non-empty it also clears the query |
| Paste | Insert pasted text |

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
- Text insertion and paste behavior in the workspace search box, repo detail filters, input prompts, and commit box are intentionally not remapped.
