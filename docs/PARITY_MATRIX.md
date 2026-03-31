# Lazygit Parity Matrix

This file is the canonical repo-mode clone-parity tracker for `super_lazygit_rust`.
It exists so reviewers can answer "what is still missing?" without doing a fresh
manual audit of upstream lazygit.

Current open clone-parity beads: none.

Source material for this matrix:

- `references/lazygit-master/README.md`
- `references/lazygit-master/docs/keybindings/Keybindings_en.md`
- `PLAN.md` sections `39. Lazygit Feature Parity Target Matrix` and `46. Test Topology Matrix`
- `.beads/issues.jsonl`

## Regression Path

Run the canonical parity harness from the repo root:

```bash
./scripts/run_lazygit_parity_regression.sh
```

That runner executes:

1. `cargo fmt --all --check`
2. `cargo check --all-targets`
3. `cargo clippy --all-targets -- -D warnings`
4. matrix/harness sync tests in `super-lazygit-app`
5. routed keybinding coverage in `super-lazygit-tui`
6. end-to-end keyboard-flow coverage in `super-lazygit-app`
7. the submodule entry/return runtime regression

If a new clone-parity gap is discovered, do not leave it implicit:

1. Create or reopen the relevant bead.
2. Add or update the matching matrix row below.
3. Add or point to the automated regression that proves the row.

## Matrix

| Surface | Upstream lazygit expectation | Current status | Bead ownership | Primary regression coverage | Notes |
| --- | --- | --- | --- | --- | --- |
| Keyboard-global controls and shared navigation | Global command palette, screen controls, recent-repo jumps, paging, tab cycling, and escape semantics match lazygit's keyboard repo-shell contract. | Shipped | `supergit-1ae`, `supergit-1ae.1`, `supergit-1ae.1.1`, `supergit-1ae.1.2` | `route_repository_global_recent_repo_command_log_and_shell_prompt_keys`, `route_repository_screen_mode_keys_cycle_modes`, `route_repository_shared_navigation_keys_cover_pages_edges_and_tabs` | This row only covers keyboard-global routing. It does not imply grouped side-window layout or mouse parity. |
| Repo-shell grouped layout and mouse interaction | Lazygit renders five side windows (`status`, `files`, `branches`, `commits`, `stash`) with grouped tabs, click-to-focus and tab-click switching, plus enlarged/full focused-side-window behavior. | Shipped | `supergit-2ri` | `route_repository_mouse_click_switches_grouped_repo_tabs`, `route_repository_mouse_click_focuses_main_repo_panes`, `render_repo_shell_shows_all_branch_graph_preview`, `render_repo_shell_hides_side_panes_in_fullscreen_mode` | The repo shell now uses grouped side windows, routed mouse clicks, and fullscreen/half-screen takeover behavior instead of the old flat tab shell. |
| Files and main-panel semantics | Status/detail panes preserve lazygit-style staging, patch/menu affordances, tree navigation, and commit/push flow expectations. | Shipped | `supergit-1ae.3`, `supergit-1ae.3.1`, `supergit-1ae.3.2` | `route_repository_status_patch_menu_key`, `route_repository_status_tree_and_shell_keys`, `route_repository_status_enter_opens_detail_in_flat_mode`, `e2e_keyboard_harness_runs_workspace_triage_commit_and_push_flow` | Treat this row as the clone gate for status-pane UX, not just file listing. |
| Commit-files and custom patch explorer | Commit-file drill-down, detached checkout, and custom patch builder behavior stay reachable from commit-history context. | Shipped | `supergit-1ae.2`, `supergit-1ae.2.1` | `repo_mode_commit_detail_enter_opens_selected_commit_files`, `repo_mode_commit_file_list_enter_opens_file_diff`, `repo_mode_commit_file_diff_enter_returns_to_file_list`, `e2e_keyboard_harness_runs_commit_history_file_and_detached_checkout_cycle` | This is the non-status diff/file surface that lazygit users hit during history review. |
| Commit-history advanced actions | Commit browsing includes filter/log menus, browser copy flows, detached checkout, and advanced history actions instead of a read-only log. | Shipped | `supergit-3o1.4`, `supergit-3o1.13` | `repo_mode_commit_detail_routes_history_shortcuts`, `route_repository_commit_filter_menu_key`, `route_repository_commit_log_options_key`, `route_repository_commit_history_o_opens_selected_commit_in_browser`, `e2e_keyboard_harness_runs_commit_history_file_and_detached_checkout_cycle` | Pair this row with the commit-files row above for full history parity. |
| Branches, remote branches, and remotes management | Local branches, remote branches, and remotes expose lazygit-style checkout, reset, PR, git-flow, fork, and sort affordances. | Shipped | `supergit-3o1.3`, `supergit-3o1.8`, `supergit-3o1.9`, `supergit-3o1.14` | `repo_mode_branch_detail_routes_selection_and_prompts`, `repo_mode_remote_branch_detail_routes_selection_and_actions`, `repo_mode_remotes_detail_routes_selection_and_actions`, `e2e_keyboard_harness_runs_remote_branch_commit_and_checkout_cycle`, `e2e_keyboard_harness_runs_remote_management_cycle` | This row absorbed the residual advanced branch/ref work from `supergit-3o1.14`. |
| Status/log graph and release-reference controls | Status-origin log graph, tag browsing, tag lifecycle, and related release-reference controls behave like lazygit instead of acting as placeholders. | Shipped | `supergit-3o1.2`, `supergit-3o1.10`, `supergit-3o1.15`, `supergit-15v` | `graph::tests::render_commit_graph_handles_merges`, `graph::tests::render_commit_graph_handles_multi_branch_continuation`, `render_repo_shell_shows_all_branch_graph_preview`, `render_repo_shell_commit_detail_expands_graph_history_window`, `repo_mode_tags_detail_routes_filter_navigation_and_actions`, `e2e_keyboard_harness_runs_tag_filter_create_push_delete_cycle` | The graph rows now come from a structured Unicode renderer in Rust rather than raw `git log --graph` strings, while tag and release-reference coverage remains in the existing TUI and end-to-end suites. |
| Stash flows | Stash creation modes, stash file drill-down, apply/pop/drop/rename/new-branch flows, and stash-side copy/browser affordances match lazygit expectations. | Shipped | `supergit-3o1.5` | `repo_mode_stash_detail_routes_selection_create_branch_rename_apply_pop_and_drop`, `e2e_keyboard_harness_inspects_stash_files_before_applying_older_stash` | The stash file inspection flow is the highest-signal regression for this cluster. |
| Reflog flows | Reflog history includes contextual commit actions and detached-checkout workflows instead of a minimal pointer list. | Shipped | `supergit-3o1.6`, `supergit-3o1.15` | `repo_mode_reflog_detail_routes_commit_context_and_history_actions`, `e2e_keyboard_harness_runs_reflog_history_and_detached_checkout_cycle` | Keep reflog parity separate from commit-history parity because the navigation source and semantics differ. |
| Worktrees | Worktree filtering, selection, creation, switching, open-in-editor, and removal stay covered as a first-class repo subview. | Shipped | `supergit-3o1.7` | `repo_mode_detail_contract_routes_filter_worktrees_and_main_return`, `repo_mode_worktree_detail_routes_selection_switch_create_open_and_remove`, `e2e_keyboard_harness_runs_repo_detail_filter_worktree_and_return_cycle` | This row also guards the return-to-parent-repo flow. |
| Submodules | Submodule drill-down, bulk maintenance menu, lifecycle actions, and nested-repo entry/return flow stay aligned with lazygit's submodule surface. | Shipped | `supergit-3o1.11`, `supergit-3o1.15`, `supergit-s8e` | `open_bulk_submodule_options_opens_menu_without_selection`, `init_all_submodules_enqueues_git_job`, `update_all_submodules_enqueues_git_job`, `update_all_submodules_recursively_enqueues_git_job`, `deinit_all_submodules_enqueues_git_job`, `route_repository_submodule_keys_cover_subview_and_actions`, `runtime_enters_and_leaves_nested_submodule_repo` | The runtime test proves the nested repo transition, while the reducer tests pin the upstream `b` bulk-actions parity. |
| Repo-interior clone audit umbrella | The remaining repo-interior lazygit clone backlog must stay explicit; no newly discovered gap may remain undocumented. | Closed | `supergit-3o1` | `docs/PARITY_MATRIX.md`, `./scripts/run_lazygit_parity_regression.sh`, `parity_matrix_lists_all_open_clone_parity_beads` | Closed once the matrix and harness made the remaining interior gap set explicit and no follow-up child bead remained necessary. |
| Canonical matrix and regression harness | The matrix and harness themselves stay versioned, repeatable, and enforced by tests so parity claims cannot drift away from beads. | Closed | `supergit-3o1.12` | `lazygit_parity_regression_script_targets_documented_suites`, `parity_matrix_lists_all_open_clone_parity_beads` | This row remains as the owner of the matrix contract even after the bead is complete. |
