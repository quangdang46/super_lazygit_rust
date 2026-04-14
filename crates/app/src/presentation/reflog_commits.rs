use std::collections::HashSet;

use crate::style::basic_styles::{fg_blue, fg_magenta};
use crate::style::text_style::TextStyle;
use crate::style::theme::{
    cherry_picked_commit_text_style, default_text_color, diff_terminal_color,
};

use super_lazygit_core::CommitItem;

pub fn get_reflog_commit_list_display_strings(
    commits: &[CommitItem],
    full_description: bool,
    cherry_picked_commit_hash_set: &HashSet<String>,
    diff_name: &str,
    now_millis: i64,
    time_format: &str,
    short_time_format: &str,
    parse_emoji: bool,
) -> Vec<Vec<String>> {
    commits
        .iter()
        .map(|commit| {
            let diffed = commit.oid == diff_name;
            let cherry_picked = cherry_picked_commit_hash_set.contains(&commit.oid);
            if full_description {
                get_full_description_display_strings_for_reflog_commit(
                    commit,
                    cherry_picked,
                    diffed,
                    parse_emoji,
                    time_format,
                    short_time_format,
                    now_millis,
                )
            } else {
                get_display_strings_for_reflog_commit(commit, cherry_picked, diffed, parse_emoji)
            }
        })
        .collect()
}

fn reflog_hash_color(cherry_picked: bool, diffed: bool) -> TextStyle {
    if diffed {
        return diff_terminal_color();
    }

    let hash_color = fg_blue();
    if cherry_picked {
        cherry_picked_commit_text_style(&[], &[])
    } else {
        hash_color
    }
}

fn get_full_description_display_strings_for_reflog_commit(
    commit: &CommitItem,
    cherry_picked: bool,
    diffed: bool,
    parse_emoji: bool,
    _time_format: &str,
    _short_time_format: &str,
    now_millis: i64,
) -> Vec<String> {
    let mut name = commit.summary.clone();
    if parse_emoji {
        name = parse_emoji_string(&name);
    }

    let short_hash = if commit.short_oid.len() < commit.oid.len() {
        commit.short_oid.clone()
    } else {
        commit.oid.clone()
    };

    vec![
        reflog_hash_color(cherry_picked, diffed).sprint(&short_hash),
        fg_magenta().sprint(&format_time(now_millis, commit.unix_timestamp)),
        default_text_color().sprint(&name),
    ]
}

fn get_display_strings_for_reflog_commit(
    commit: &CommitItem,
    cherry_picked: bool,
    diffed: bool,
    parse_emoji: bool,
) -> Vec<String> {
    let mut name = commit.summary.clone();
    if parse_emoji {
        name = parse_emoji_string(&name);
    }

    let short_hash = if commit.short_oid.len() < commit.oid.len() {
        commit.short_oid.clone()
    } else {
        commit.oid.clone()
    };

    vec![
        reflog_hash_color(cherry_picked, diffed).sprint(&short_hash),
        default_text_color().sprint(&name),
    ]
}

fn format_time(now_millis: i64, timestamp: i64) -> String {
    let diff_sec = (now_millis / 1000) - timestamp;
    let diff_min = diff_sec / 60;
    let diff_hours = diff_min / 60;
    let diff_days = diff_hours / 24;

    if diff_days > 365 {
        format!("{} years ago", diff_days / 365)
    } else if diff_days > 30 {
        format!("{} months ago", diff_days / 30)
    } else if diff_days > 0 {
        format!("{} days ago", diff_days)
    } else if diff_hours > 0 {
        format!("{} hours ago", diff_hours)
    } else if diff_min > 0 {
        format!("{} minutes ago", diff_min)
    } else {
        "just now".to_string()
    }
}

fn parse_emoji_string(s: &str) -> String {
    s.to_string()
}
