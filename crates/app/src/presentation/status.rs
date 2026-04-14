use std::time::SystemTime;

use crate::i18n::TranslationSet;
use crate::presentation::icons::{icon_for_branch, is_icon_enabled, linked_worktree_icon};
use crate::presentation::item_operation_to_string;
use crate::presentation::loader::{loader, SpinnerConfig};
use crate::style::basic_styles::{fg_cyan, fg_yellow};
use crate::style::text_style::TextStyle;
use crate::style::theme::default_text_color;
use super_lazygit_core::state::BranchItem;
use super_lazygit_tui::presentation::ItemOperation;

pub fn format_status(
    repo_name: String,
    current_branch: &BranchItem,
    item_operation: ItemOperation,
    linked_worktree_name: &str,
    working_tree_state: super_lazygit_core::state::WorkingTreeState,
    tr: &TranslationSet,
) -> String {
    let mut status = String::new();

    if current_branch.is_real_branch() {
        let branch_status = branch_status(current_branch, item_operation, tr, SystemTime::now());
        if !branch_status.is_empty() {
            status.push_str(&branch_status);
            status.push(' ');
        }
    }

    if working_tree_state.any() {
        status.push_str(
            &fg_yellow().sprint(format!("({}) ", working_tree_state.lower_case_title(tr))),
        );
    }

    let name = get_branch_text_style_styled(&current_branch.name).sprint(&current_branch.name);
    if !linked_worktree_name.is_empty() {
        let icon = if is_icon_enabled() {
            format!("{} ", linked_worktree_icon())
        } else {
            String::new()
        };
        repo_name = format!(
            "{}({}{})",
            repo_name,
            icon,
            fg_cyan().sprint(linked_worktree_name)
        );
    }
    status.push_str(&format!("{} → {}", repo_name, name));

    status
}

fn get_branch_text_style_styled(name: &str) -> TextStyle {
    crate::presentation::branches::get_branch_text_style(name)
}

fn branch_status(
    branch: &BranchItem,
    item_operation: ItemOperation,
    tr: &TranslationSet,
    now: SystemTime,
) -> String {
    let item_operation_str = item_operation_to_string(item_operation, tr);
    if !item_operation_str.is_empty() {
        let now_millis = now
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        return fg_cyan().sprintf(
            "%s %s",
            &[
                &item_operation_str,
                &loader(now_millis, &SpinnerConfig::default()),
            ],
        );
    }
    String::new()
}
