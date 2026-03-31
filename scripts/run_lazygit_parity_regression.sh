#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

echo "[parity] format + compile gates"
cargo fmt --all --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings

echo "[parity] matrix and harness drift guards"
cargo test -p super-lazygit-app lazygit_parity_regression_script_targets_documented_suites
cargo test -p super-lazygit-app parity_matrix_lists_all_open_clone_parity_beads

echo "[parity] routed lazygit key and panel coverage"
cargo test -p super-lazygit-tui route_repository_
cargo test -p super-lazygit-tui repo_mode_

echo "[parity] end-to-end repo-mode workflow coverage"
cargo test -p super-lazygit-app e2e_keyboard_harness_runs_ -- --nocapture
cargo test -p super-lazygit-app e2e_keyboard_harness_inspects_stash_files_before_applying_older_stash -- --nocapture
cargo test -p super-lazygit-app runtime_enters_and_leaves_nested_submodule_repo -- --nocapture
