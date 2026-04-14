use crate::i18n::TranslationSet;
use crate::presentation::icons::{icon_for_worktree, is_icon_enabled, linked_worktree_icon};
use crate::style::basic_styles::{fg_cyan, fg_default, fg_red, fg_yellow};
use crate::style::text_style::TextStyle;
use crate::style::theme::default_text_color;
use crate::utils::formatting::short_hash;
use super_lazygit_core::state::WorktreeItem;

pub fn get_worktree_display_strings(
    tr: &TranslationSet,
    worktrees: &[WorktreeItem],
) -> Vec<Vec<String>> {
    worktrees
        .iter()
        .map(|worktree| get_worktree_display_string(tr, worktree))
        .collect()
}

pub fn get_worktree_display_string(tr: &TranslationSet, worktree: &WorktreeItem) -> Vec<String> {
    let mut text_style = default_text_color();

    let current = if worktree.is_current { "  *" } else { "" };
    let current_color = if worktree.is_current {
        fg_cyan()
    } else {
        fg_cyan()
    };

    let icon = icon_for_worktree(worktree.is_path_missing);
    if worktree.is_path_missing {
        text_style = fg_red();
    }

    let mut res = Vec::new();
    res.push(current_color.sprint(current));
    if is_icon_enabled() {
        res.push(text_style.sprint(&icon));
    }

    let mut name = worktree.name.clone();
    if worktree.is_path_missing && !is_icon_enabled() {
        name.push_str(&format!(" {}", tr.missing_worktree));
    }
    res.push(text_style.sprint(&name));

    let branch = if let Some(ref branch_name) = worktree.branch {
        fg_cyan().sprint(branch_name)
    } else if !worktree.head.is_empty() {
        fg_yellow().sprint(format!("HEAD detached at {}", short_hash(&worktree.head)))
    } else {
        String::new()
    };

    res.push(format!("{}{}", branch, main_worktree_label(tr, worktree)));
    res
}

fn main_worktree_label(tr: &TranslationSet, worktree: &WorktreeItem) -> String {
    if worktree.is_main {
        fg_default().sprint(format!(" {}", tr.main_worktree))
    } else {
        String::new()
    }
}
