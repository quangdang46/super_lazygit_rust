// Ported from ./references/lazygit-master/pkg/gui/presentation/reflog_commits.go

use ratatui::style::{Color, Style};
use ratatui::text::Span;

use super_lazygit_core::CommitItem;

/// Get display strings for commit list (full description mode).
/// Parity: getFullDescriptionDisplayStringsForReflogCommit in Go
pub fn get_reflog_commit_display_strings(
    commit: &CommitItem,
    cherry_picked: bool,
    diffed: bool,
    _parse_emoji: bool,
    _time_format: &str,
    _short_time_format: &str,
    now_millis: i64,
) -> Vec<Span<'static>> {
    // Parity: reflogHashColor in Go
    // - diffed: theme.DiffTerminalColor = style.FgMagenta
    // - cherry_picked: theme.CherryPickedCommitTextStyle (complex style)
    // - default: style.FgBlue
    let hash_color = if diffed {
        Color::Magenta // DiffTerminalColor
    } else if cherry_picked {
        Color::Magenta // CherryPickedCommitTextStyle (simplified)
    } else {
        Color::Blue // style.FgBlue
    };

    let short_hash = commit.short_oid.clone();
    let time_str = format_time(now_millis, commit.unix_timestamp);
    let name = commit.summary.clone();

    vec![
        Span::styled(short_hash, Style::default().fg(hash_color)),
        Span::styled(time_str, Style::default().fg(Color::Magenta)), // style.FgMagenta
        Span::raw(name), // theme.DefaultTextColor
    ]
}

/// Get display strings for commit list (compact version).
/// Parity: getDisplayStringsForReflogCommit in Go
pub fn get_reflog_commit_display_strings_compact(
    commit: &CommitItem,
    cherry_picked: bool,
    diffed: bool,
) -> Vec<Span<'static>> {
    let hash_color = if diffed {
        Color::Magenta // DiffTerminalColor
    } else if cherry_picked {
        Color::Magenta // CherryPickedCommitTextStyle
    } else {
        Color::Blue
    };

    let short_hash = commit.short_oid.clone();
    let name = commit.summary.clone();

    vec![
        Span::styled(short_hash, Style::default().fg(hash_color)),
        Span::raw(name), // theme.DefaultTextColor
    ]
}

fn format_time(now_millis: i64, timestamp: i64) -> String {
    // Parity: utils.UnixToDateSmart in Go
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