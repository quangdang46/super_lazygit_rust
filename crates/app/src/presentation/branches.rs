use regex::Regex;
use std::collections::HashMap;
use std::sync::RwLock;

use crate::i18n::TranslationSet;
use crate::presentation::icons::{icon_for_branch, is_icon_enabled, linked_worktree_icon};
use crate::presentation::item_operation_to_string;
use crate::presentation::loader::{loader, SpinnerConfig};
use crate::style::basic_styles::{fg_cyan, fg_default, fg_green, fg_magenta, fg_red, fg_yellow};
use crate::style::text_style::TextStyle;
use crate::style::theme::default_text_color;
use super_lazygit_core::state::{BranchItem, WorktreeItem};
use super_lazygit_tui::presentation::ItemOperation;
use unicode_width::UnicodeWidthStr;

const COMMIT_HASH_SHORT_SIZE: usize = 8;

struct ColorMatcher {
    patterns: HashMap<String, TextStyle>,
    is_regex: bool,
}

static COLOR_PATTERNS: RwLock<Option<ColorMatcher>> = RwLock::new(None);

pub fn get_branch_list_display_strings(
    branches: &[BranchItem],
    get_item_operation: impl Fn(&dyn crate::types::common::HasUrn) -> ItemOperation,
    full_description: bool,
    diff_name: &str,
    view_width: usize,
    tr: &TranslationSet,
    worktrees: &[WorktreeItem],
    show_commit_hash: bool,
) -> Vec<Vec<String>> {
    branches
        .iter()
        .map(|branch| {
            let diffed = branch.name == diff_name;
            get_branch_display_strings(
                branch,
                get_item_operation(branch),
                full_description,
                diffed,
                view_width,
                tr,
                worktrees,
                show_commit_hash,
                std::time::SystemTime::now(),
            )
        })
        .collect()
}

fn get_branch_display_strings(
    b: &BranchItem,
    item_operation: ItemOperation,
    full_description: bool,
    diffed: bool,
    view_width: usize,
    tr: &TranslationSet,
    worktrees: &[WorktreeItem],
    show_commit_hash: bool,
    now: std::time::SystemTime,
) -> Vec<String> {
    let checked_out_by_worktree = b.checked_out_by_other_worktree(worktrees);
    let branch_status = branch_status(b, item_operation, tr, now);
    let divergence = divergence_str(b, item_operation, tr);

    let mut available_width = view_width.saturating_sub(4);
    if !divergence.is_empty() {
        available_width = available_width.saturating_sub(string_width(&divergence) + 1);
    }
    if is_icon_enabled() {
        available_width = available_width.saturating_sub(2);
    }
    if show_commit_hash {
        available_width = available_width.saturating_sub(COMMIT_HASH_SHORT_SIZE + 1);
    }
    let mut padding_needed_for_divergence = available_width;

    let mut display_name = b.name.clone();
    if let Some(ref display_name_override) = b.display_name {
        display_name = display_name_override.clone();
    }

    if !branch_status.is_empty() {
        available_width =
            available_width.saturating_sub(string_width(&decolorise(&branch_status)) + 1);
    }

    let mut worktree_icon = String::new();
    if checked_out_by_worktree {
        if let Some(ref wt) = b.worktree_for_branch(worktrees) {
            if wt.name != b.name {
                if is_icon_enabled() {
                    worktree_icon = format!("({} {})", linked_worktree_icon(), wt.name);
                } else {
                    worktree_icon = format!("({} {})", tr.worktree, wt.name);
                }

                let remaining = available_width
                    .saturating_sub(string_width(&worktree_icon))
                    .saturating_sub(1);
                if remaining < string_width(&display_name) {
                    if is_icon_enabled() {
                        worktree_icon = linked_worktree_icon();
                    } else {
                        worktree_icon = format!("({})", tr.worktree);
                    }
                }
            }
        } else {
            if is_icon_enabled() {
                worktree_icon = linked_worktree_icon();
            } else {
                worktree_icon = format!("({})", tr.worktree);
            }
        }

        available_width = available_width.saturating_sub(string_width(&worktree_icon) + 1);
    }

    let mut name_text_style = get_branch_text_style(&b.name);
    if diffed {
        name_text_style = crate::style::theme::diff_terminal_color();
    }

    if string_width(&display_name) > available_width.max(3) {
        let len = available_width.max(4);
        display_name = truncate_with_ellipsis(&display_name, len);
    }
    let mut colored_name = name_text_style.sprint(&display_name);
    if checked_out_by_worktree {
        colored_name = format!("{} {}", colored_name, fg_default().sprint(&worktree_icon));
    }
    if !branch_status.is_empty() {
        colored_name = format!("{} {}", colored_name, branch_status);
    }

    let recency_color = if b.recency == "  *" {
        fg_green()
    } else {
        fg_cyan()
    };

    let mut res = Vec::with_capacity(6);
    res.push(recency_color.sprint(&b.recency));

    if is_icon_enabled() {
        res.push(name_text_style.sprint(&icon_for_branch(b)));
    }

    if show_commit_hash {
        res.push(short_hash(&b.commit_hash));
    }

    if !divergence.is_empty() {
        if full_description {
            padding_needed_for_divergence = 1;
        } else {
            padding_needed_for_divergence = padding_needed_for_divergence
                .saturating_sub(string_width(&decolorise(&colored_name)))
                .saturating_sub(1);
        }
        if padding_needed_for_divergence > 0 {
            colored_name.push_str(&" ".repeat(padding_needed_for_divergence));
            colored_name.push_str(&fg_cyan().sprint(&divergence));
        }
    }
    res.push(colored_name);

    if full_description {
        let upstream_remote = b.upstream_remote.as_deref().unwrap_or("");
        let upstream_branch = b.upstream_branch.as_deref().unwrap_or("");
        res.push(format!(
            "{} {}",
            fg_yellow().sprint(upstream_remote),
            fg_yellow().sprint(upstream_branch)
        ));
        res.push(truncate_with_ellipsis(&b.subject, 60));
    }

    res
}

pub fn get_branch_text_style(name: &str) -> TextStyle {
    if let Some(ref matcher) = *COLOR_PATTERNS.read().unwrap() {
        if let Some(style) = matcher.match_name(name) {
            return style;
        }
    }
    default_text_color()
}

impl ColorMatcher {
    fn match_name(&self, name: &str) -> Option<TextStyle> {
        if self.is_regex {
            for (pattern, style) in &self.patterns {
                if Regex::new(pattern)
                    .map(|re| re.is_match(name))
                    .unwrap_or(false)
                {
                    return Some(*style);
                }
            }
        } else {
            let branch_type = name.split('/').next().unwrap_or(name);
            if let Some(style) = self.patterns.get(branch_type) {
                return Some(*style);
            }
        }
        None
    }
}

fn branch_status(
    branch: &BranchItem,
    item_operation: ItemOperation,
    tr: &TranslationSet,
    now: std::time::SystemTime,
) -> String {
    let item_operation_str = item_operation_to_string(item_operation, tr);
    if !item_operation_str.is_empty() {
        let now_millis = now
            .duration_since(std::time::UNIX_EPOCH)
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

    let mut result = String::new();
    if branch.is_tracking_remote() {
        if branch.upstream_gone {
            result = fg_red().sprint(&tr.upstream_gone);
        } else if branch.matches_upstream() {
            result = fg_green().sprint("✓");
        } else if branch.remote_branch_not_stored_locally() {
            result = fg_magenta().sprint("?");
        } else if branch.is_behind_for_pull() && branch.is_ahead_for_pull() {
            result =
                fg_yellow().sprintf("↓{}↑{}", &[&branch.behind_for_pull, &branch.ahead_for_pull]);
        } else if branch.is_behind_for_pull() {
            result = fg_yellow().sprintf("↓{}", &[&branch.behind_for_pull]);
        } else if branch.is_ahead_for_pull() {
            result = fg_yellow().sprintf("↑{}", &[&branch.ahead_for_pull]);
        }
    }

    result
}

fn divergence_str(
    branch: &BranchItem,
    item_operation: ItemOperation,
    tr: &TranslationSet,
) -> String {
    let mut result = String::new();
    if item_operation_to_string(item_operation, tr).is_empty() {
        let behind = branch.behind_base_branch;
        if behind != 0 {
            result.push_str(&format!("↓{}", behind));
        }
    }
    result
}

pub fn set_custom_branches(custom_branch_colors: HashMap<String, String>, is_regex: bool) {
    let patterns = custom_branch_colors
        .into_iter()
        .map(|(k, v)| {
            let style = match v.as_str() {
                "red" => fg_red(),
                "green" => fg_green(),
                "yellow" => fg_yellow(),
                "blue" => fg_blue(),
                "magenta" => fg_magenta(),
                "cyan" => fg_cyan(),
                _ => default_text_color(),
            };
            (k, style)
        })
        .collect();
    *COLOR_PATTERNS.write().unwrap() = Some(ColorMatcher { patterns, is_regex });
}

fn string_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

fn decolorise(s: &str) -> String {
    let re = Regex::new(r"\x1b\[[0-9;]*m").unwrap();
    re.replace_all(s, "").to_string()
}

fn short_hash(hash: &str) -> String {
    if hash.len() < COMMIT_HASH_SHORT_SIZE {
        hash.to_string()
    } else {
        hash[..COMMIT_HASH_SHORT_SIZE].to_string()
    }
}

fn truncate_with_ellipsis(s: &str, max_width: usize) -> String {
    let width = string_width(s);
    if width <= max_width {
        return s.to_string();
    }

    let mut result = String::new();
    let mut current_width = 0;
    for ch in s.chars() {
        let ch_width = UnicodeWidthStr::width(ch.to_string().as_str());
        if current_width + ch_width + 1 > max_width {
            break;
        }
        result.push(ch);
        current_width += ch_width;
    }
    result.push('…');
    result
}
