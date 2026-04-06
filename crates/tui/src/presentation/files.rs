//! File tree presentation for the TUI.
//!
//! Ports `lazygit/pkg/gui/presentation/files.go` to Rust with lazygit parity.
//! Provides file status formatting, line change display, commit file display,
//! change status coloring, and tree arrow constants.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use super_lazygit_core::{escape_special_chars, FileStatusKind};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Expanded directory arrow.
///
/// Parity: `EXPANDED_ARROW` in `presentation/files.go`.
pub const EXPANDED_ARROW: &str = "\u{25bc}"; // ▼

/// Collapsed directory arrow.
///
/// Parity: `COLLAPSED_ARROW` in `presentation/files.go`.
pub const COLLAPSED_ARROW: &str = "\u{25b6}"; // ▶

// ---------------------------------------------------------------------------
// Patch status (for commit file views)
// ---------------------------------------------------------------------------

/// Patch inclusion status for a commit file.
///
/// Parity: `patch.PatchStatus` enum in Go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PatchStatus {
    #[default]
    Unselected,
    Whole,
    Part,
}

// ---------------------------------------------------------------------------
// File status formatting
// ---------------------------------------------------------------------------

/// Format the two-character file status with appropriate colors.
///
/// Parity: `formatFileStatus` in `presentation/files.go`.
/// The first char represents the staged status, the second the unstaged status.
/// Returns a pair of styled Spans for the two characters.
pub fn format_file_status(short_status: &str, rest_color: Color) -> Vec<Span<'static>> {
    if short_status.len() < 2 {
        return vec![Span::styled(
            short_status.to_string(),
            Style::default().fg(rest_color),
        )];
    }

    let first_char = &short_status[0..1];
    let first_color = match first_char {
        "?" => Color::Red, // UnstagedChangesColor
        " " => rest_color,
        _ => Color::Green,
    };

    let second_char = &short_status[1..2];
    let second_color = if second_char == " " {
        rest_color
    } else {
        Color::Red // UnstagedChangesColor
    };

    vec![
        Span::styled(first_char.to_string(), Style::default().fg(first_color)),
        Span::styled(second_char.to_string(), Style::default().fg(second_color)),
    ]
}

/// Format line change statistics (numstat).
///
/// Parity: `formatLineChanges` in `presentation/files.go`.
/// Returns empty string if both are zero.
pub fn format_line_changes(lines_added: u32, lines_deleted: u32) -> String {
    let mut output = String::new();

    if lines_added != 0 {
        output.push_str(&format!("+{lines_added}"));
    }

    if lines_deleted != 0 {
        if !output.is_empty() {
            output.push(' ');
        }
        output.push_str(&format!("-{lines_deleted}"));
    }

    output
}

/// Get styled spans for line change statistics.
///
/// Parity: `formatLineChanges` in `presentation/files.go` with color.
pub fn format_line_changes_styled(lines_added: u32, lines_deleted: u32) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    if lines_added != 0 {
        spans.push(Span::styled(
            format!("+{lines_added}"),
            Style::default().fg(Color::Green),
        ));
    }

    if lines_deleted != 0 {
        if !spans.is_empty() {
            spans.push(Span::raw(" "));
        }
        spans.push(Span::styled(
            format!("-{lines_deleted}"),
            Style::default().fg(Color::Red),
        ));
    }

    spans
}

// ---------------------------------------------------------------------------
// File line rendering
// ---------------------------------------------------------------------------

/// Options for rendering a file tree entry.
pub struct FileLineOptions<'a> {
    pub is_collapsed: bool,
    pub has_unstaged_changes: bool,
    pub has_staged_changes: bool,
    pub visual_depth: usize,
    pub show_numstat: bool,
    pub name: &'a str,
    pub short_status: &'a str,
    pub is_file: bool,
    pub is_submodule: bool,
    pub is_worktree: bool,
    pub lines_added: u32,
    pub lines_deleted: u32,
    pub is_rename: bool,
    pub previous_name: Option<&'a str>,
}

/// Render a single file tree entry as a styled Line.
///
/// Parity: `getFileLine` in `presentation/files.go`.
pub fn get_file_line(opts: &FileLineOptions<'_>) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(8);

    let name_color = if opts.has_staged_changes && !opts.has_unstaged_changes {
        Color::Green
    } else if opts.has_staged_changes {
        Color::Yellow
    } else {
        Color::Reset // DefaultTextColor
    };

    let indentation = "  ".repeat(opts.visual_depth);

    if !opts.is_file {
        // Directory node
        let arrow = if opts.is_collapsed {
            COLLAPSED_ARROW
        } else {
            EXPANDED_ARROW
        };
        spans.push(Span::raw(indentation));
        spans.push(Span::styled(
            arrow.to_string(),
            Style::default().fg(name_color),
        ));
        spans.push(Span::styled(
            " ".to_string(),
            Style::default().fg(name_color),
        ));
    } else {
        // File node
        spans.push(Span::raw(indentation));
        spans.extend(format_file_status(opts.short_status, name_color));
        spans.push(Span::styled(
            " ".to_string(),
            Style::default().fg(name_color),
        ));
    }

    // File name (with special char escaping)
    let display_name = if opts.is_rename {
        if let Some(prev) = opts.previous_name {
            format!(
                "{} \u{2192} {}",
                escape_special_chars(prev),
                escape_special_chars(opts.name)
            )
        } else {
            escape_special_chars(opts.name)
        }
    } else {
        escape_special_chars(opts.name)
    };
    spans.push(Span::styled(display_name, Style::default().fg(name_color)));

    // Submodule indicator
    if opts.is_submodule {
        spans.push(Span::raw(" (submodule)"));
    }

    // Numstat
    if opts.is_file && opts.show_numstat {
        let changes = format_line_changes(opts.lines_added, opts.lines_deleted);
        if !changes.is_empty() {
            spans.push(Span::raw(" "));
            spans.extend(format_line_changes_styled(
                opts.lines_added,
                opts.lines_deleted,
            ));
        }
    }

    Line::from(spans)
}

// ---------------------------------------------------------------------------
// Commit file rendering
// ---------------------------------------------------------------------------

/// Render a single commit file tree entry as a styled Line.
///
/// Parity: `getCommitFileLine` in `presentation/files.go`.
pub fn get_commit_file_line(
    is_collapsed: bool,
    visual_depth: usize,
    name: &str,
    is_directory: bool,
    status: PatchStatus,
    change_status: Option<&str>,
) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(4);

    let indentation = "  ".repeat(visual_depth);
    let name_color = match status {
        PatchStatus::Whole => Color::Green,
        PatchStatus::Part => Color::Yellow,
        PatchStatus::Unselected => Color::Reset,
    };

    spans.push(Span::raw(indentation));

    if is_directory {
        let arrow = if is_collapsed {
            COLLAPSED_ARROW
        } else {
            EXPANDED_ARROW
        };
        spans.push(Span::styled(
            arrow.to_string(),
            Style::default().fg(name_color),
        ));
        spans.push(Span::raw(" "));
    } else {
        // Symbol based on patch status
        let (symbol, symbol_color) = match status {
            PatchStatus::Whole => ("\u{25cf}", name_color), // ●
            PatchStatus::Part => ("\u{25d0}", name_color),  // ◐
            PatchStatus::Unselected => {
                let cs = change_status.unwrap_or("");
                (cs, get_color_for_change_status(cs))
            }
        };
        spans.push(Span::styled(
            symbol.to_string(),
            Style::default().fg(symbol_color),
        ));
        spans.push(Span::raw(" "));
    }

    // Escaped file name
    let escaped_name = escape_special_chars(name);
    spans.push(Span::styled(escaped_name, Style::default().fg(name_color)));

    Line::from(spans)
}

/// Get the color for a file change status character.
///
/// Parity: `getColorForChangeStatus` in `presentation/files.go`.
pub fn get_color_for_change_status(change_status: &str) -> Color {
    match change_status {
        "A" => Color::Green,
        "M" | "R" => Color::Yellow,
        "D" => Color::Red, // UnstagedChangesColor
        "C" => Color::Cyan,
        "T" => Color::Magenta,
        _ => Color::Reset, // DefaultTextColor
    }
}

/// Map a `FileStatusKind` to a display color.
///
/// Companion helper for the Rust-side `FileStatusKind` enum.
pub fn file_status_kind_color(kind: FileStatusKind) -> Color {
    match kind {
        FileStatusKind::Modified => Color::Yellow,
        FileStatusKind::Added => Color::Green,
        FileStatusKind::Deleted => Color::Red,
        FileStatusKind::Renamed => Color::Yellow,
        FileStatusKind::Untracked => Color::Red,
        FileStatusKind::Conflicted => Color::Red,
    }
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Extract a file name at a given tree depth.
///
/// Parity: `fileNameAtDepth` in `presentation/files.go`.
/// Splits the internal path by `/` and joins from `depth` onward.
pub fn file_name_at_depth(internal_path: &str, depth: usize) -> String {
    let parts: Vec<&str> = internal_path.split('/').collect();
    let mut adjusted_depth = depth;

    if adjusted_depth == 0 && parts.first() == Some(&".") {
        if parts.len() == 1 {
            return "/".to_string();
        }
        adjusted_depth = 1;
    }

    if adjusted_depth >= parts.len() {
        return parts.last().unwrap_or(&"").to_string();
    }

    parts[adjusted_depth..].join("/")
}

/// Extract a commit file name at a given tree depth.
///
/// Parity: `commitFileNameAtDepth` in `presentation/files.go`.
pub fn commit_file_name_at_depth(internal_path: &str, depth: usize) -> String {
    file_name_at_depth(internal_path, depth)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_file_status_modified_staged() {
        let spans = format_file_status("M ", Color::Reset);
        assert_eq!(spans.len(), 2);
    }

    #[test]
    fn format_file_status_untracked() {
        let spans = format_file_status("??", Color::Reset);
        assert_eq!(spans.len(), 2);
    }

    #[test]
    fn format_file_status_short_input() {
        let spans = format_file_status("M", Color::Reset);
        assert_eq!(spans.len(), 1);
    }

    #[test]
    fn format_line_changes_both_nonzero() {
        assert_eq!(format_line_changes(5, 3), "+5 -3");
    }

    #[test]
    fn format_line_changes_only_added() {
        assert_eq!(format_line_changes(10, 0), "+10");
    }

    #[test]
    fn format_line_changes_only_deleted() {
        assert_eq!(format_line_changes(0, 7), "-7");
    }

    #[test]
    fn format_line_changes_both_zero() {
        assert!(format_line_changes(0, 0).is_empty());
    }

    #[test]
    fn color_for_change_status_matches_go() {
        assert_eq!(get_color_for_change_status("A"), Color::Green);
        assert_eq!(get_color_for_change_status("M"), Color::Yellow);
        assert_eq!(get_color_for_change_status("R"), Color::Yellow);
        assert_eq!(get_color_for_change_status("D"), Color::Red);
        assert_eq!(get_color_for_change_status("C"), Color::Cyan);
        assert_eq!(get_color_for_change_status("T"), Color::Magenta);
        assert_eq!(get_color_for_change_status("X"), Color::Reset);
    }

    #[test]
    fn file_name_at_depth_basic() {
        assert_eq!(file_name_at_depth("src/gui/main.go", 0), "src/gui/main.go");
        assert_eq!(file_name_at_depth("src/gui/main.go", 1), "gui/main.go");
        assert_eq!(file_name_at_depth("src/gui/main.go", 2), "main.go");
    }

    #[test]
    fn file_name_at_depth_dot_prefix() {
        assert_eq!(file_name_at_depth("./src/main.go", 0), "src/main.go");
        assert_eq!(file_name_at_depth(".", 0), "/");
    }

    #[test]
    fn file_name_at_depth_beyond_end() {
        assert_eq!(file_name_at_depth("src/main.go", 10), "main.go");
    }

    #[test]
    fn get_file_line_directory_collapsed() {
        let opts = FileLineOptions {
            is_collapsed: true,
            has_unstaged_changes: false,
            has_staged_changes: false,
            visual_depth: 0,
            show_numstat: false,
            name: "src",
            short_status: "",
            is_file: false,
            is_submodule: false,
            is_worktree: false,
            lines_added: 0,
            lines_deleted: 0,
            is_rename: false,
            previous_name: None,
        };
        let line = get_file_line(&opts);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains(COLLAPSED_ARROW));
        assert!(text.contains("src"));
    }

    #[test]
    fn get_file_line_directory_expanded() {
        let opts = FileLineOptions {
            is_collapsed: false,
            has_unstaged_changes: false,
            has_staged_changes: false,
            visual_depth: 0,
            show_numstat: false,
            name: "src",
            short_status: "",
            is_file: false,
            is_submodule: false,
            is_worktree: false,
            lines_added: 0,
            lines_deleted: 0,
            is_rename: false,
            previous_name: None,
        };
        let line = get_file_line(&opts);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains(EXPANDED_ARROW));
    }

    #[test]
    fn get_file_line_file_with_submodule() {
        let opts = FileLineOptions {
            is_collapsed: false,
            has_unstaged_changes: true,
            has_staged_changes: false,
            visual_depth: 1,
            show_numstat: false,
            name: "vendor/lib",
            short_status: " M",
            is_file: true,
            is_submodule: true,
            is_worktree: false,
            lines_added: 0,
            lines_deleted: 0,
            is_rename: false,
            previous_name: None,
        };
        let line = get_file_line(&opts);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("(submodule)"));
    }

    #[test]
    fn get_file_line_with_numstat() {
        let opts = FileLineOptions {
            is_collapsed: false,
            has_unstaged_changes: true,
            has_staged_changes: false,
            visual_depth: 0,
            show_numstat: true,
            name: "file.rs",
            short_status: " M",
            is_file: true,
            is_submodule: false,
            is_worktree: false,
            lines_added: 10,
            lines_deleted: 3,
            is_rename: false,
            previous_name: None,
        };
        let line = get_file_line(&opts);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("+10"));
        assert!(text.contains("-3"));
    }

    #[test]
    fn commit_file_line_whole_patch() {
        let line = get_commit_file_line(false, 0, "main.go", false, PatchStatus::Whole, None);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("\u{25cf}")); // ●
        assert!(text.contains("main.go"));
    }

    #[test]
    fn commit_file_line_partial_patch() {
        let line = get_commit_file_line(false, 0, "main.go", false, PatchStatus::Part, None);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("\u{25d0}")); // ◐
    }

    #[test]
    fn commit_file_line_unselected_with_change() {
        let line = get_commit_file_line(
            false,
            0,
            "main.go",
            false,
            PatchStatus::Unselected,
            Some("M"),
        );
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("M"));
    }

    #[test]
    fn commit_file_line_directory() {
        let line = get_commit_file_line(false, 1, "src", true, PatchStatus::Unselected, None);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains(EXPANDED_ARROW));
        assert!(text.contains("src"));
    }

    #[test]
    fn file_status_kind_color_matches_expectations() {
        assert_eq!(
            file_status_kind_color(FileStatusKind::Modified),
            Color::Yellow
        );
        assert_eq!(file_status_kind_color(FileStatusKind::Added), Color::Green);
        assert_eq!(file_status_kind_color(FileStatusKind::Deleted), Color::Red);
        assert_eq!(
            file_status_kind_color(FileStatusKind::Renamed),
            Color::Yellow
        );
        assert_eq!(
            file_status_kind_color(FileStatusKind::Untracked),
            Color::Red
        );
        assert_eq!(
            file_status_kind_color(FileStatusKind::Conflicted),
            Color::Red
        );
    }
}
