//! Branch list presentation for the TUI.
//!
//! Ports `lazygit/pkg/gui/presentation/branches.go` to Rust with lazygit parity.
//! Provides branch display formatting with recency, name, tracking status,
//! divergence indicators, worktree markers, and color pattern matching.

use std::collections::HashMap;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super_lazygit_core::{BranchItem, WorktreeItem};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Short commit hash display length.
///
/// Parity: `utils.COMMIT_HASH_SHORT_SIZE` in Go.
const COMMIT_HASH_SHORT_SIZE: usize = 8;

// ---------------------------------------------------------------------------
// Branch color matching
// ---------------------------------------------------------------------------

/// Color matcher for branch names based on configured patterns.
///
/// Parity: `colorMatcher` struct in `presentation/branches.go`.
#[derive(Debug, Clone, Default)]
pub struct BranchColorMatcher {
    patterns: HashMap<String, Color>,
    is_regex: bool,
}

impl BranchColorMatcher {
    /// Match a branch name against configured color patterns.
    ///
    /// Parity: `colorMatcher.match` in `presentation/branches.go`.
    pub fn match_color(&self, name: &str) -> Option<Color> {
        if self.is_regex {
            for (pattern, color) in &self.patterns {
                if regex_matches(pattern, name) {
                    return Some(*color);
                }
            }
        } else {
            // Old behavior: match on branch type (prefix before first /)
            let branch_type = name.split('/').next().unwrap_or(name);
            if let Some(color) = self.patterns.get(branch_type) {
                return Some(*color);
            }
        }
        None
    }
}

/// Simple regex match helper (prefix-based for non-regex mode).
fn regex_matches(pattern: &str, text: &str) -> bool {
    // Simplified regex matching - uses contains for basic pattern support.
    // Full parity would use the `regex` crate.
    if pattern.starts_with('^') && pattern.ends_with('$') {
        // Exact match
        let inner = &pattern[1..pattern.len() - 1];
        text == inner
    } else if pattern.starts_with('^') {
        text.starts_with(&pattern[1..])
    } else if pattern.ends_with('$') {
        text.ends_with(&pattern[..pattern.len() - 1])
    } else {
        text.contains(pattern)
    }
}

/// Global branch color patterns (thread-local for simplicity).
///
/// Parity: `colorPatterns` global in `presentation/branches.go`.
static DEFAULT_MATCHER: std::sync::LazyLock<BranchColorMatcher> =
    std::sync::LazyLock::new(BranchColorMatcher::default);

/// Set custom branch color patterns.
///
/// Parity: `SetCustomBranches` in `presentation/branches.go`.
/// Returns a new `BranchColorMatcher` that callers can use for display.
pub fn create_branch_color_matcher(
    custom_colors: HashMap<String, Color>,
    is_regex: bool,
) -> BranchColorMatcher {
    BranchColorMatcher {
        patterns: custom_colors,
        is_regex,
    }
}

// ---------------------------------------------------------------------------
// Display options
// ---------------------------------------------------------------------------

/// Options for branch list display.
///
/// Groups all the parameters that `GetBranchListDisplayStrings` receives in Go.
pub struct BranchDisplayOptions<'a> {
    pub branches: &'a [BranchItem],
    pub full_description: bool,
    pub diff_name: &'a str,
    pub worktrees: &'a [WorktreeItem],
    pub show_commit_hash: bool,
    pub show_divergence_from_base: DivergenceDisplay,
    pub color_matcher: Option<&'a BranchColorMatcher>,
}

/// How to display divergence from base branch.
///
/// Parity: `gui.showDivergenceFromBaseBranch` config in Go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DivergenceDisplay {
    #[default]
    None,
    Arrow,
    ArrowAndNumber,
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Get display lines for a list of branches.
///
/// Parity: `GetBranchListDisplayStrings` in `presentation/branches.go`.
pub fn get_branch_list_display_strings(opts: &BranchDisplayOptions<'_>) -> Vec<Line<'static>> {
    opts.branches
        .iter()
        .map(|branch| {
            let diffed = branch.name == opts.diff_name;
            get_branch_display_string(
                branch,
                opts.full_description,
                diffed,
                opts.worktrees,
                opts.show_commit_hash,
                opts.show_divergence_from_base,
                opts.color_matcher,
            )
        })
        .collect()
}

/// Format a single branch as a styled Line.
///
/// Parity: `getBranchDisplayStrings` in `presentation/branches.go`.
pub fn get_branch_display_string(
    branch: &BranchItem,
    full_description: bool,
    diffed: bool,
    worktrees: &[WorktreeItem],
    show_commit_hash: bool,
    divergence_display: DivergenceDisplay,
    color_matcher: Option<&BranchColorMatcher>,
) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(8);

    let checked_out_by_worktree = branch.checked_out_by_other_worktree(worktrees);
    let branch_status = branch_status_text(branch);
    let divergence = divergence_str(branch, divergence_display);

    let display_name = branch
        .display_name
        .as_deref()
        .unwrap_or(&branch.name)
        .to_string();

    // Branch name style
    let name_style = if diffed {
        Style::default().fg(Color::Cyan) // DiffTerminalColor
    } else {
        let matcher = color_matcher.unwrap_or(&DEFAULT_MATCHER);
        match matcher.match_color(&branch.name) {
            Some(color) => Style::default().fg(color),
            None => Style::default(), // DefaultTextColor
        }
    };

    // Column 1: Recency
    let recency_color = if branch.recency == "  *" {
        Color::Green
    } else {
        Color::Cyan
    };
    spans.push(Span::styled(
        branch.recency.clone(),
        Style::default().fg(recency_color),
    ));

    // Column 2: Commit hash (optional)
    if show_commit_hash || full_description {
        let short = short_hash(&branch.commit_hash);
        spans.push(Span::raw(" "));
        spans.push(Span::raw(short));
    }

    // Column 3: Branch name with worktree icon and status
    spans.push(Span::raw(" "));

    let mut name_text = display_name.clone();

    // Worktree indicator
    if checked_out_by_worktree {
        let worktree_icon = if let Some(wt) = branch.worktree_for_branch(worktrees) {
            if wt.name != branch.name {
                format!("(worktree {})", wt.name)
            } else {
                "(worktree)".to_string()
            }
        } else {
            "(worktree)".to_string()
        };
        name_text = format!("{name_text} {worktree_icon}");
    }

    // Branch status (tracking info)
    if !branch_status.is_empty() {
        name_text = format!("{name_text} {branch_status}");
    }

    spans.push(Span::styled(name_text, name_style));

    // Divergence from base branch
    if !divergence.is_empty() {
        spans.push(Span::styled(
            format!(" {divergence}"),
            Style::default().fg(Color::Cyan),
        ));
    }

    // Full description extras: upstream remote/branch, subject
    if full_description {
        let upstream_remote = branch.upstream_remote.as_deref().unwrap_or("");
        let upstream_branch = branch.upstream_branch.as_deref().unwrap_or("");
        if !upstream_remote.is_empty() || !upstream_branch.is_empty() {
            spans.push(Span::styled(
                format!(" {upstream_remote} {upstream_branch}"),
                Style::default().fg(Color::Yellow),
            ));
        }

        if !branch.subject.is_empty() {
            let truncated = truncate_with_ellipsis(&branch.subject, 60);
            spans.push(Span::raw(format!(" {truncated}")));
        }
    }

    Line::from(spans)
}

// ---------------------------------------------------------------------------
// Branch status
// ---------------------------------------------------------------------------

/// Get tracking status text for a branch.
///
/// Parity: `BranchStatus` in `presentation/branches.go`.
/// Returns a styled status string (without item operation handling, which
/// is a TUI-layer concern).
pub fn branch_status_text(branch: &BranchItem) -> String {
    if !branch.is_tracking_remote() {
        return String::new();
    }

    if branch.upstream_gone {
        return "\u{2191}gone".to_string(); // ↑gone
    }

    if branch.matches_upstream() {
        return "\u{2713}".to_string(); // ✓
    }

    if branch.remote_branch_not_stored_locally() {
        return "?".to_string();
    }

    let is_behind = branch.is_behind_for_pull();
    let is_ahead = branch.is_ahead_for_pull();

    if is_behind && is_ahead {
        format!(
            "\u{2193}{}\u{2191}{}",
            branch.behind_for_pull, branch.ahead_for_pull
        )
    } else if is_behind {
        format!("\u{2193}{}", branch.behind_for_pull)
    } else if is_ahead {
        format!("\u{2191}{}", branch.ahead_for_pull)
    } else {
        String::new()
    }
}

/// Get styled spans for branch tracking status.
///
/// Parity: `BranchStatus` color application in `presentation/branches.go`.
pub fn branch_status_styled(branch: &BranchItem) -> Option<Span<'static>> {
    if !branch.is_tracking_remote() {
        return None;
    }

    if branch.upstream_gone {
        return Some(Span::styled(
            "gone".to_string(),
            Style::default().fg(Color::Red),
        ));
    }

    if branch.matches_upstream() {
        return Some(Span::styled(
            "\u{2713}".to_string(),
            Style::default().fg(Color::Green),
        ));
    }

    if branch.remote_branch_not_stored_locally() {
        return Some(Span::styled(
            "?".to_string(),
            Style::default().fg(Color::Magenta),
        ));
    }

    let is_behind = branch.is_behind_for_pull();
    let is_ahead = branch.is_ahead_for_pull();

    if is_behind && is_ahead {
        Some(Span::styled(
            format!(
                "\u{2193}{}\u{2191}{}",
                branch.behind_for_pull, branch.ahead_for_pull
            ),
            Style::default().fg(Color::Yellow),
        ))
    } else if is_behind {
        Some(Span::styled(
            format!("\u{2193}{}", branch.behind_for_pull),
            Style::default().fg(Color::Yellow),
        ))
    } else if is_ahead {
        Some(Span::styled(
            format!("\u{2191}{}", branch.ahead_for_pull),
            Style::default().fg(Color::Yellow),
        ))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Divergence
// ---------------------------------------------------------------------------

/// Get divergence string from base branch.
///
/// Parity: `divergenceStr` in `presentation/branches.go`.
fn divergence_str(branch: &BranchItem, display: DivergenceDisplay) -> String {
    if display == DivergenceDisplay::None {
        return String::new();
    }

    let behind = branch.behind_base_branch;
    if behind == 0 {
        return String::new();
    }

    match display {
        DivergenceDisplay::ArrowAndNumber => format!("\u{2193}{behind}"),
        DivergenceDisplay::Arrow => "\u{2193}".to_string(),
        DivergenceDisplay::None => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Branch text style
// ---------------------------------------------------------------------------

/// Get the text style for a branch name based on color patterns.
///
/// Parity: `GetBranchTextStyle` in `presentation/branches.go`.
pub fn get_branch_text_style(name: &str, matcher: Option<&BranchColorMatcher>) -> Style {
    let matcher = matcher.unwrap_or(&DEFAULT_MATCHER);
    match matcher.match_color(name) {
        Some(color) => Style::default().fg(color),
        None => Style::default(),
    }
}

/// Get the row style for a branch in a list.
///
/// Provides selection and head-branch highlighting.
pub fn branch_row_style(branch: &BranchItem, is_selected: bool, is_focused: bool) -> Style {
    let mut style = if branch.is_head {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    if is_selected {
        style = style.add_modifier(Modifier::BOLD);
        if is_focused {
            style = style.add_modifier(Modifier::REVERSED);
        }
    }

    style
}

// ---------------------------------------------------------------------------
// Utility helpers
// ---------------------------------------------------------------------------

/// Truncate a hash to short display length.
fn short_hash(hash: &str) -> String {
    if hash.len() > COMMIT_HASH_SHORT_SIZE {
        hash[..COMMIT_HASH_SHORT_SIZE].to_string()
    } else {
        hash.to_string()
    }
}

/// Truncate a string with ellipsis if it exceeds max_len.
///
/// Parity: `utils.TruncateWithEllipsis` in Go.
fn truncate_with_ellipsis(s: &str, max_len: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_len {
        return s.to_string();
    }
    if max_len <= 1 {
        return "\u{2026}".to_string(); // …
    }
    let mut truncated: String = s.chars().take(max_len - 1).collect();
    truncated.push('\u{2026}');
    truncated
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_branch(name: &str) -> BranchItem {
        BranchItem {
            name: name.to_string(),
            recency: "2h".to_string(),
            commit_hash: "abc1234567890def".to_string(),
            ..BranchItem::default()
        }
    }

    fn make_tracking_branch(
        name: &str,
        ahead: &str,
        behind: &str,
        upstream_gone: bool,
    ) -> BranchItem {
        BranchItem {
            name: name.to_string(),
            recency: "1h".to_string(),
            commit_hash: "abc1234567890def".to_string(),
            upstream_remote: Some("origin".to_string()),
            upstream_branch: Some(name.to_string()),
            ahead_for_pull: ahead.to_string(),
            behind_for_pull: behind.to_string(),
            ahead_for_push: String::new(),
            behind_for_push: String::new(),
            upstream_gone,
            ..BranchItem::default()
        }
    }

    #[test]
    fn branch_status_upstream_gone() {
        let b = make_tracking_branch("main", "0", "0", true);
        let status = branch_status_text(&b);
        assert!(status.contains("gone"));
    }

    #[test]
    fn branch_status_matches_upstream() {
        let b = make_tracking_branch("main", "0", "0", false);
        let status = branch_status_text(&b);
        assert_eq!(status, "\u{2713}"); // ✓
    }

    #[test]
    fn branch_status_ahead_and_behind() {
        let b = make_tracking_branch("feature", "3", "2", false);
        let status = branch_status_text(&b);
        assert!(status.contains("\u{2193}2")); // ↓2
        assert!(status.contains("\u{2191}3")); // ↑3
    }

    #[test]
    fn branch_status_only_ahead() {
        let b = make_tracking_branch("feature", "5", "0", false);
        let status = branch_status_text(&b);
        assert_eq!(status, "\u{2191}5"); // ↑5
    }

    #[test]
    fn branch_status_only_behind() {
        let b = make_tracking_branch("feature", "0", "3", false);
        let status = branch_status_text(&b);
        assert_eq!(status, "\u{2193}3"); // ↓3
    }

    #[test]
    fn branch_status_remote_not_stored() {
        let b = make_tracking_branch("feature", "?", "?", false);
        let status = branch_status_text(&b);
        assert_eq!(status, "?");
    }

    #[test]
    fn branch_status_no_tracking() {
        let b = make_branch("local-only");
        let status = branch_status_text(&b);
        assert!(status.is_empty());
    }

    #[test]
    fn divergence_str_none() {
        let b = BranchItem {
            behind_base_branch: 5,
            ..make_branch("feature")
        };
        assert!(divergence_str(&b, DivergenceDisplay::None).is_empty());
    }

    #[test]
    fn divergence_str_arrow_only() {
        let b = BranchItem {
            behind_base_branch: 5,
            ..make_branch("feature")
        };
        assert_eq!(divergence_str(&b, DivergenceDisplay::Arrow), "\u{2193}");
    }

    #[test]
    fn divergence_str_arrow_and_number() {
        let b = BranchItem {
            behind_base_branch: 5,
            ..make_branch("feature")
        };
        assert_eq!(
            divergence_str(&b, DivergenceDisplay::ArrowAndNumber),
            "\u{2193}5"
        );
    }

    #[test]
    fn divergence_str_zero_behind() {
        let b = BranchItem {
            behind_base_branch: 0,
            ..make_branch("feature")
        };
        assert!(divergence_str(&b, DivergenceDisplay::ArrowAndNumber).is_empty());
    }

    #[test]
    fn branch_display_basic() {
        let branch = make_branch("feature/awesome");
        let line = get_branch_display_string(
            &branch,
            false,
            false,
            &[],
            false,
            DivergenceDisplay::None,
            None,
        );
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("feature/awesome"));
        assert!(text.contains("2h"));
    }

    #[test]
    fn branch_display_with_hash() {
        let branch = make_branch("main");
        let line = get_branch_display_string(
            &branch,
            false,
            false,
            &[],
            true,
            DivergenceDisplay::None,
            None,
        );
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("abc12345"));
    }

    #[test]
    fn branch_display_diffed() {
        let branch = make_branch("main");
        let line = get_branch_display_string(
            &branch,
            false,
            true, // diffed
            &[],
            false,
            DivergenceDisplay::None,
            None,
        );
        // Should use Cyan style for diffed branch
        let has_cyan = line
            .spans
            .iter()
            .any(|s| s.style.fg == Some(Color::Cyan) && s.content.contains("main"));
        assert!(has_cyan);
    }

    #[test]
    fn branch_display_full_description() {
        let branch = BranchItem {
            subject: "Add new feature for users".to_string(),
            upstream_remote: Some("origin".to_string()),
            upstream_branch: Some("main".to_string()),
            ..make_tracking_branch("main", "0", "0", false)
        };
        let line = get_branch_display_string(
            &branch,
            true, // full_description
            false,
            &[],
            true,
            DivergenceDisplay::None,
            None,
        );
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("origin"));
        assert!(text.contains("Add new feature"));
    }

    #[test]
    fn branch_list_returns_correct_count() {
        let branches = vec![
            make_branch("main"),
            make_branch("dev"),
            make_branch("feature"),
        ];
        let opts = BranchDisplayOptions {
            branches: &branches,
            full_description: false,
            diff_name: "",
            worktrees: &[],
            show_commit_hash: false,
            show_divergence_from_base: DivergenceDisplay::None,
            color_matcher: None,
        };
        let lines = get_branch_list_display_strings(&opts);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn color_matcher_prefix_mode() {
        let mut patterns = HashMap::new();
        patterns.insert("feature".to_string(), Color::Green);
        patterns.insert("hotfix".to_string(), Color::Red);
        let matcher = BranchColorMatcher {
            patterns,
            is_regex: false,
        };
        assert_eq!(matcher.match_color("feature/awesome"), Some(Color::Green));
        assert_eq!(matcher.match_color("hotfix/urgent"), Some(Color::Red));
        assert_eq!(matcher.match_color("main"), None);
    }

    #[test]
    fn color_matcher_regex_mode() {
        let mut patterns = HashMap::new();
        patterns.insert("^release".to_string(), Color::Magenta);
        let matcher = BranchColorMatcher {
            patterns,
            is_regex: true,
        };
        assert_eq!(matcher.match_color("release/v1.0"), Some(Color::Magenta));
        assert_eq!(matcher.match_color("feature/release"), None);
    }

    #[test]
    fn truncate_with_ellipsis_short_string() {
        assert_eq!(truncate_with_ellipsis("hello", 10), "hello");
    }

    #[test]
    fn truncate_with_ellipsis_exact_length() {
        assert_eq!(truncate_with_ellipsis("hello", 5), "hello");
    }

    #[test]
    fn truncate_with_ellipsis_long_string() {
        let result = truncate_with_ellipsis("hello world", 8);
        assert_eq!(result.chars().count(), 8);
        assert!(result.ends_with('\u{2026}'));
    }

    #[test]
    fn short_hash_truncates() {
        assert_eq!(short_hash("abc1234567890def"), "abc12345");
        assert_eq!(short_hash("short"), "short");
    }

    #[test]
    fn branch_row_style_head() {
        let branch = BranchItem {
            is_head: true,
            ..make_branch("main")
        };
        let style = branch_row_style(&branch, false, false);
        assert_eq!(style.fg, Some(Color::Green));
    }

    #[test]
    fn branch_row_style_selected() {
        let branch = make_branch("feature");
        let style = branch_row_style(&branch, true, true);
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }
}
