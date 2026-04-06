//! Worktree list presentation for the TUI.
//!
//! Ports `lazygit/pkg/gui/presentation/worktrees.go` to Rust with lazygit parity.
//! Provides worktree display formatting with current-indicator, branch info,
//! missing-path handling, and main-worktree labels.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use super_lazygit_core::WorktreeItem;

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Get display lines for a list of worktrees.
///
/// Parity: `GetWorktreeDisplayStrings` in `presentation/worktrees.go`.
/// Returns one `Line` per worktree, each containing styled column spans.
pub fn get_worktree_display_strings(worktrees: &[WorktreeItem]) -> Vec<Line<'static>> {
    worktrees.iter().map(get_worktree_display_string).collect()
}

/// Format a single worktree as a styled Line.
///
/// Parity: `GetWorktreeDisplayString` in `presentation/worktrees.go`.
/// Columns: current-indicator | name (+missing) | branch-info | main-label
pub fn get_worktree_display_string(worktree: &WorktreeItem) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(4);

    let text_style = if worktree.is_path_missing {
        Style::default().fg(Color::Red)
    } else {
        Style::default()
    };

    // Column 1: Current indicator
    // Parity: current = "  *" with green, else "" with cyan
    let (current_text, current_color) = if worktree.is_current {
        ("  *", Color::Green)
    } else {
        ("", Color::Cyan)
    };
    spans.push(Span::styled(
        current_text.to_string(),
        Style::default().fg(current_color),
    ));

    // Column 2: Name (with missing indicator when icons are not enabled)
    // Parity: when IsPathMissing and no icons, append " (missing worktree)"
    let mut name = worktree.name.clone();
    if worktree.is_path_missing {
        name.push_str(" (missing worktree)");
    }
    spans.push(Span::styled(format!(" {name}"), text_style));

    // Column 3: Branch info
    // Parity: branch name in cyan, or "HEAD detached at <short>" in yellow
    if let Some(branch) = &worktree.branch {
        if !branch.is_empty() {
            spans.push(Span::styled(
                format!(" {branch}"),
                Style::default().fg(Color::Cyan),
            ));
        }
    } else if !worktree.head.is_empty() {
        let short = short_hash(&worktree.head);
        spans.push(Span::styled(
            format!(" HEAD detached at {short}"),
            Style::default().fg(Color::Yellow),
        ));
    }

    // Column 4: Main worktree label
    // Parity: `mainWorktreeLabel` in Go
    let main_label = main_worktree_label(worktree);
    if !main_label.is_empty() {
        spans.push(Span::raw(main_label));
    }

    Line::from(spans)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Parity: `mainWorktreeLabel` in `presentation/worktrees.go`.
fn main_worktree_label(worktree: &WorktreeItem) -> String {
    if worktree.is_main {
        " (main worktree)".to_string()
    } else {
        String::new()
    }
}

/// Truncate a hash to short display length (8 chars).
///
/// Parity: `utils.ShortHash` in Go.
fn short_hash(hash: &str) -> String {
    const SHORT_HASH_SIZE: usize = 8;
    if hash.len() > SHORT_HASH_SIZE {
        hash[..SHORT_HASH_SIZE].to_string()
    } else {
        hash.to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_worktree(name: &str, branch: Option<&str>, is_current: bool) -> WorktreeItem {
        WorktreeItem {
            path: PathBuf::from(format!("/tmp/{name}")),
            branch: branch.map(ToString::to_string),
            head: String::new(),
            name: name.to_string(),
            is_main: false,
            is_current,
            is_path_missing: false,
            git_dir: None,
        }
    }

    #[test]
    fn current_worktree_shows_asterisk() {
        let wt = make_worktree("mywork", Some("feature-x"), true);
        let line = get_worktree_display_string(&wt);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("*"));
        assert!(text.contains("feature-x"));
    }

    #[test]
    fn non_current_worktree_no_asterisk() {
        let wt = make_worktree("other", Some("develop"), false);
        let line = get_worktree_display_string(&wt);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(!text.contains("*"));
        assert!(text.contains("develop"));
    }

    #[test]
    fn missing_path_shows_indicator() {
        let wt = WorktreeItem {
            is_path_missing: true,
            name: "gone".to_string(),
            ..make_worktree("gone", Some("old"), false)
        };
        let line = get_worktree_display_string(&wt);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("(missing worktree)"));
    }

    #[test]
    fn detached_head_shows_short_hash() {
        let wt = WorktreeItem {
            branch: None,
            head: "abc1234567890def".to_string(),
            ..make_worktree("detached", None, false)
        };
        let line = get_worktree_display_string(&wt);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("HEAD detached at abc12345"));
    }

    #[test]
    fn main_worktree_shows_label() {
        let wt = WorktreeItem {
            is_main: true,
            ..make_worktree("main", Some("main"), false)
        };
        let line = get_worktree_display_string(&wt);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("(main worktree)"));
    }

    #[test]
    fn list_returns_correct_count() {
        let worktrees = vec![
            make_worktree("a", Some("main"), true),
            make_worktree("b", Some("dev"), false),
        ];
        let lines = get_worktree_display_strings(&worktrees);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn empty_list_returns_empty() {
        let lines = get_worktree_display_strings(&[]);
        assert!(lines.is_empty());
    }

    #[test]
    fn short_hash_truncates_correctly() {
        assert_eq!(short_hash("abc1234567890def"), "abc12345");
        assert_eq!(short_hash("short"), "short");
        assert_eq!(short_hash(""), "");
    }
}
