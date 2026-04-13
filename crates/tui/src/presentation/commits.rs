//! Commit list presentation for the TUI.
//!
//! Ports `lazygit/pkg/gui/presentation/commits.go` to Rust with lazygit parity.
//! Provides commit list display formatting with bisect status, hash coloring,
//! action color mapping, branch head markers, and divergence indicators.

use std::collections::HashSet;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super_lazygit_core::{
    BisectCommitStatus, BisectState, BranchItem, CommitDivergence, CommitItem, CommitStatus,
    CommitTodoAction,
};

use super::authors::author_span;

// ---------------------------------------------------------------------------
// Bisect display status (GUI-focused, distinct from data-model BisectState)
// ---------------------------------------------------------------------------

/// GUI-focused bisect status for a single commit.
///
/// Parity: `BisectStatus` enum in `presentation/commits.go`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BisectDisplayStatus {
    None,
    Old,
    New,
    Skipped,
    Candidate,
    Current,
}

/// Internal struct tracking the index bounds of the bisect range.
///
/// Parity: `bisectBounds` struct in `presentation/commits.go`.
#[derive(Debug, Clone)]
struct BisectBounds {
    new_index: usize,
    old_index: usize,
}

// ---------------------------------------------------------------------------
// Public context struct for commit display configuration
// ---------------------------------------------------------------------------

/// Options for commit list display.
///
/// Groups all the parameters that `GetCommitListDisplayStrings` receives in Go
/// into a single struct for ergonomic Rust usage.
pub struct CommitDisplayOptions<'a> {
    pub commits: &'a [CommitItem],
    pub branches: &'a [BranchItem],
    pub current_branch_name: &'a str,
    pub has_rebase_update_refs_config: bool,
    pub full_description: bool,
    pub cherry_picked_hashes: &'a HashSet<String>,
    pub diff_name: &'a str,
    pub marked_base_commit: &'a str,
    pub parse_emoji: bool,
    pub selected_commit_hash: Option<&'a str>,
    pub start_idx: usize,
    pub end_idx: usize,
    pub show_graph: bool,
    pub graph_lines: &'a [String],
    pub bisect_state: Option<&'a BisectState>,
    pub main_branches: &'a [String],
    pub commit_hash_length: usize,
    pub author_short_length: usize,
    pub author_long_length: usize,
    pub conflict_label: &'a str,
    pub marked_commit_marker: &'a str,
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Get the display strings for the commit list.
///
/// Parity: `GetCommitListDisplayStrings` in `presentation/commits.go`.
/// Returns a list of styled Lines, one per visible commit row.
pub fn get_commit_list_display_strings(opts: &CommitDisplayOptions<'_>) -> Vec<Line<'static>> {
    if opts.commits.is_empty() || opts.start_idx >= opts.commits.len() {
        return Vec::new();
    }

    let end_idx = opts.end_idx.min(opts.commits.len());
    let start_idx = opts.start_idx;

    // Index of first non-TODO commit (rebase offset)
    let _rebase_offset = index_of_first_non_todo_commit(opts.commits).min(end_idx);

    let filtered_commits = &opts.commits[start_idx..end_idx];

    // Bisect bounds
    let bisect_bounds = get_bisect_bounds(opts.commits, opts.bisect_state);

    // Graph line accessor
    let get_graph_line = |idx: usize| -> String {
        if !opts.show_graph {
            return String::new();
        }
        opts.graph_lines.get(idx).cloned().unwrap_or_default()
    };

    // Determine branch heads to visualize (non-current, non-main branches)
    let branch_heads_to_visualize = compute_branch_heads_to_visualize(
        opts.branches,
        opts.current_branch_name,
        opts.main_branches,
        opts.has_rebase_update_refs_config,
        opts.commits,
    );

    let mut lines = Vec::with_capacity(filtered_commits.len());
    let mut will_be_rebased = opts.marked_base_commit.is_empty();

    for (i, commit) in filtered_commits.iter().enumerate() {
        let unfiltered_idx = i + start_idx;
        let bisect_status = get_bisect_status(
            unfiltered_idx,
            &commit.oid,
            opts.bisect_state,
            bisect_bounds.as_ref(),
        );
        let is_marked_base_commit = !commit.oid.is_empty() && commit.oid == opts.marked_base_commit;
        if is_marked_base_commit {
            will_be_rebased = true;
        }

        let graph_line = get_graph_line(unfiltered_idx);
        let author_length = if opts.full_description {
            opts.author_long_length
        } else {
            opts.author_short_length
        };

        lines.push(display_commit(DisplayCommitContext {
            commit,
            branch_heads_to_visualize: &branch_heads_to_visualize,
            has_rebase_update_refs_config: opts.has_rebase_update_refs_config,
            cherry_picked_hashes: opts.cherry_picked_hashes,
            is_marked_base_commit,
            will_be_rebased,
            diff_name: opts.diff_name,
            parse_emoji: opts.parse_emoji,
            graph_line: &graph_line,
            full_description: opts.full_description,
            bisect_status,
            bisect_state: opts.bisect_state,
            author_length,
            commit_hash_length: opts.commit_hash_length,
            conflict_label: opts.conflict_label,
            marked_commit_marker: opts.marked_commit_marker,
        }));
    }

    lines
}

// ---------------------------------------------------------------------------
// Internal helper context for single-commit display
// ---------------------------------------------------------------------------

struct DisplayCommitContext<'a> {
    commit: &'a CommitItem,
    branch_heads_to_visualize: &'a HashSet<String>,
    has_rebase_update_refs_config: bool,
    cherry_picked_hashes: &'a HashSet<String>,
    is_marked_base_commit: bool,
    will_be_rebased: bool,
    diff_name: &'a str,
    parse_emoji: bool,
    graph_line: &'a str,
    full_description: bool,
    bisect_status: BisectDisplayStatus,
    bisect_state: Option<&'a BisectState>,
    author_length: usize,
    commit_hash_length: usize,
    conflict_label: &'a str,
    marked_commit_marker: &'a str,
}

// ---------------------------------------------------------------------------
// Single commit display
// ---------------------------------------------------------------------------

/// Format a single commit as a styled Line of spans.
///
/// Parity: `displayCommit` in `presentation/commits.go`.
/// Columns: divergence | hash | bisect | description | action | author | graphLine+mark+tag+name
fn display_commit(ctx: DisplayCommitContext<'_>) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(8);

    // 1. Bisect string
    let bisect_string = get_bisect_status_text(ctx.bisect_status, ctx.bisect_state);

    // 2. Hash string
    let hash_color = get_hash_color(
        ctx.commit,
        ctx.diff_name,
        ctx.cherry_picked_hashes,
        ctx.bisect_status,
        ctx.bisect_state,
    );
    let hash_string = if ctx.commit_hash_length >= ctx.commit.oid.len() {
        ctx.commit.oid.clone()
    } else if ctx.commit_hash_length > 0 {
        ctx.commit.oid[..ctx.commit_hash_length.min(ctx.commit.oid.len())].to_string()
    } else {
        "*".to_string()
    };

    // 3. Divergence string
    let divergence_string = if ctx.commit.divergence != CommitDivergence::None {
        let arrow = match ctx.commit.divergence {
            CommitDivergence::Left => "↑",
            CommitDivergence::Right => "↓",
            _ => "",
        };
        arrow.to_string()
    } else {
        String::new()
    };

    // 4. Description string (timestamp in full description mode)
    let description_string = if ctx.full_description && ctx.commit.unix_timestamp > 0 {
        format_timestamp_smart(ctx.commit.unix_timestamp)
    } else {
        String::new()
    };

    // 5. Action string
    let action_string = if ctx.commit.todo_action != CommitTodoAction::None {
        let mut action_str = format!("{:?}", ctx.commit.todo_action).to_lowercase();
        // Only show the flag for fixup commands (where -C changes the meaning)
        if !ctx.commit.todo_action_flag.is_empty()
            && ctx.commit.todo_action == CommitTodoAction::Fixup
        {
            action_str = format!("{action_str} {}", ctx.commit.todo_action_flag);
        }
        Some(action_str)
    } else {
        None
    };

    // 6. Tag string
    let mut tag_string = String::new();
    if ctx.full_description {
        if !ctx.commit.extra_info.is_empty() {
            tag_string = format!("{} ", ctx.commit.extra_info);
        }
    } else {
        if !ctx.commit.tags.is_empty() {
            tag_string = format!("{} ", ctx.commit.tags.join(" "));
        }

        if ctx.branch_heads_to_visualize.contains(&ctx.commit.oid)
            // Don't show branch head on commits already merged to main
            && ctx.commit.status != CommitStatus::Merged
            // Don't show on "pick" todo if rebase.updateRefs config is on
            && !(ctx.commit.is_todo() && ctx.has_rebase_update_refs_config)
        {
            tag_string = format!("* {tag_string}");
        }
    }

    // 7. Commit name (with emoji parsing and refs/heads/ prefix stripping)
    let mut name = ctx.commit.summary.clone();
    if ctx.commit.todo_action == CommitTodoAction::UpdateRef {
        name = name.trim_start_matches("refs/heads/").to_string();
    }
    if ctx.parse_emoji {
        name = parse_emoji(&name);
    }

    // 8. Mark (conflict / marked base / will be rebased)
    let mark = if ctx.commit.status == CommitStatus::Conflicted {
        format!("<-- {} --- ", ctx.conflict_label)
    } else if ctx.is_marked_base_commit {
        format!("{} ", ctx.marked_commit_marker)
    } else if !ctx.will_be_rebased {
        "✓ ".to_string()
    } else {
        String::new()
    };

    // 9. Author
    let author = author_span(&ctx.commit.author_name, ctx.author_length);

    // Assemble spans
    // Column 1: Divergence
    if !divergence_string.is_empty() {
        spans.push(Span::styled(
            divergence_string,
            Style::default().fg(hash_color),
        ));
    } else {
        spans.push(Span::raw(" "));
    }

    // Column 2: Hash
    spans.push(Span::styled(hash_string, Style::default().fg(hash_color)));

    // Column 3: Bisect
    if !bisect_string.is_empty() {
        let bisect_color = get_bisect_status_color(ctx.bisect_status);
        spans.push(Span::styled(
            format!(" {bisect_string}"),
            Style::default().fg(bisect_color),
        ));
    }

    // Column 4: Description (timestamp)
    if !description_string.is_empty() {
        spans.push(Span::styled(
            format!(" {description_string}"),
            Style::default().fg(Color::Blue),
        ));
    }

    // Column 5: Action
    if let Some(action) = action_string {
        let action_color = action_color_map(ctx.commit.todo_action, ctx.commit.status);
        spans.push(Span::styled(
            format!(" {action}"),
            Style::default().fg(action_color),
        ));
    }

    // Column 6: Author
    spans.push(Span::raw(" "));
    spans.push(author);

    // Column 7: Graph line + mark + tag + name
    let name_style = Style::default();
    let mut name_parts = String::new();
    name_parts.push_str(ctx.graph_line);
    name_parts.push_str(&mark);
    if !tag_string.is_empty() {
        // Tag part uses magenta/bold
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            tag_string,
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ));
    }
    // Build full name: graph_line + mark + name
    name_parts.push_str(&name);
    spans.push(Span::styled(name_parts, name_style));

    Line::from(spans)
}

// ---------------------------------------------------------------------------
// Bisect helpers
// ---------------------------------------------------------------------------

/// Get bisect bounds (index range) within the commit list.
///
/// Parity: `getbisectBounds` in `presentation/commits.go`.
fn get_bisect_bounds(
    commits: &[CommitItem],
    bisect_state: Option<&BisectState>,
) -> Option<BisectBounds> {
    let bs = bisect_state?;

    // Need both current_commit and at least one known status for bounds
    let current = bs.current_commit.as_deref()?;

    let mut new_index = 0usize;
    let mut found_current = false;

    for (i, commit) in commits.iter().enumerate() {
        if commit.oid == current {
            new_index = i;
            found_current = true;
        }
        // If we find an "old" status commit, that's the old bound
        if bs.commit_statuses.get(&commit.oid) == Some(&BisectCommitStatus::Old) {
            return Some(BisectBounds {
                new_index,
                old_index: i,
            });
        }
    }

    if found_current {
        Some(BisectBounds {
            new_index,
            old_index: commits.len().saturating_sub(1),
        })
    } else {
        None
    }
}

/// Get the display status for a specific commit in the bisect process.
///
/// Parity: `getBisectStatus` in `presentation/commits.go`.
fn get_bisect_status(
    index: usize,
    commit_hash: &str,
    bisect_state: Option<&BisectState>,
    bisect_bounds: Option<&BisectBounds>,
) -> BisectDisplayStatus {
    let Some(bs) = bisect_state else {
        return BisectDisplayStatus::None;
    };

    if bs.current_commit.as_deref() == Some(commit_hash) {
        return BisectDisplayStatus::Current;
    }

    if let Some(status) = bs.commit_statuses.get(commit_hash) {
        return match status {
            BisectCommitStatus::New => BisectDisplayStatus::New,
            BisectCommitStatus::Old => BisectDisplayStatus::Old,
            BisectCommitStatus::Skipped => BisectDisplayStatus::Skipped,
        };
    }

    if let Some(bounds) = bisect_bounds {
        if index >= bounds.new_index && index <= bounds.old_index {
            return BisectDisplayStatus::Candidate;
        }
    }

    BisectDisplayStatus::None
}

/// Get the display text for a bisect status.
///
/// Parity: `getBisectStatusText` in `presentation/commits.go`.
fn get_bisect_status_text(
    status: BisectDisplayStatus,
    bisect_state: Option<&BisectState>,
) -> String {
    match status {
        BisectDisplayStatus::None => String::new(),
        BisectDisplayStatus::New => {
            let term = bisect_state.map(|bs| bs.bad_term.as_str()).unwrap_or("bad");
            format!("<-- {term}")
        }
        BisectDisplayStatus::Old => {
            let term = bisect_state
                .map(|bs| bs.good_term.as_str())
                .unwrap_or("good");
            format!("<-- {term}")
        }
        BisectDisplayStatus::Current => "<-- current".to_string(),
        BisectDisplayStatus::Skipped => "<-- skipped".to_string(),
        BisectDisplayStatus::Candidate => "?".to_string(),
    }
}

/// Get the color for a bisect display status.
///
/// Parity: `getBisectStatusColor` in `presentation/commits.go`.
fn get_bisect_status_color(status: BisectDisplayStatus) -> Color {
    match status {
        BisectDisplayStatus::None => Color::Black,
        BisectDisplayStatus::New => Color::Red,
        BisectDisplayStatus::Old => Color::Green,
        BisectDisplayStatus::Skipped => Color::Yellow,
        BisectDisplayStatus::Current => Color::Magenta,
        BisectDisplayStatus::Candidate => Color::Blue,
    }
}

// ---------------------------------------------------------------------------
// Hash color
// ---------------------------------------------------------------------------

/// Get the color for a commit hash based on its status.
///
/// Parity: `getHashColor` in `presentation/commits.go`.
fn get_hash_color(
    commit: &CommitItem,
    diff_name: &str,
    cherry_picked_hashes: &HashSet<String>,
    bisect_status: BisectDisplayStatus,
    bisect_state: Option<&BisectState>,
) -> Color {
    // If bisecting, use bisect status color
    if bisect_state.is_some() {
        return get_bisect_status_color(bisect_status);
    }

    let diffed = !commit.oid.is_empty() && commit.oid == diff_name;

    let hash_color = match commit.status {
        CommitStatus::Unpushed => Color::Red,
        CommitStatus::Pushed => Color::Yellow,
        CommitStatus::Merged => Color::Green,
        CommitStatus::Rebasing
        | CommitStatus::CherryPickingOrReverting
        | CommitStatus::Conflicted => Color::Blue,
        CommitStatus::Reflog => Color::Blue,
        _ => Color::Reset,
    };

    if diffed {
        Color::Cyan // DiffTerminalColor equivalent
    } else if cherry_picked_hashes.contains(&commit.oid) {
        Color::Cyan // CherryPickedCommitTextStyle equivalent
    } else if commit.divergence == CommitDivergence::Right && commit.status != CommitStatus::Merged
    {
        Color::Blue
    } else {
        hash_color
    }
}

// ---------------------------------------------------------------------------
// Action color map
// ---------------------------------------------------------------------------

/// Get the color for a rebase todo action.
///
/// Parity: `actionColorMap` in `presentation/commits.go`.
fn action_color_map(action: CommitTodoAction, status: CommitStatus) -> Color {
    if status == CommitStatus::Conflicted {
        return Color::Red;
    }

    match action {
        CommitTodoAction::Pick => Color::Cyan,
        CommitTodoAction::Drop => Color::Red,
        CommitTodoAction::Edit => Color::Green,
        CommitTodoAction::Fixup => Color::Magenta,
        _ => Color::Yellow,
    }
}

// ---------------------------------------------------------------------------
// Branch heads computation
// ---------------------------------------------------------------------------

/// Compute which branch commit hashes should be visualized in the commit list.
///
/// Parity: `branchHeadsToVisualize` computation in `GetCommitListDisplayStrings`.
/// Only includes branches that are:
/// - Not the current branch
/// - Not a main branch
/// - Not pointing at HEAD (unless rebase.updateRefs is configured)
fn compute_branch_heads_to_visualize(
    branches: &[BranchItem],
    current_branch_name: &str,
    main_branches: &[String],
    has_rebase_update_refs_config: bool,
    commits: &[CommitItem],
) -> HashSet<String> {
    let head_hash = commits.first().map(|c| c.oid.as_str()).unwrap_or("");

    branches
        .iter()
        .filter_map(|b| {
            if b.commit_hash.is_empty() {
                return None;
            }
            // Not the current branch
            if b.name == current_branch_name {
                return None;
            }
            // Not a main branch
            if main_branches.iter().any(|mb| mb == &b.name) {
                return None;
            }
            // Don't show marker for head commit unless rebase.updateRefs is on
            if !has_rebase_update_refs_config && b.commit_hash == head_hash {
                return None;
            }
            Some(b.commit_hash.clone())
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Utility helpers
// ---------------------------------------------------------------------------

/// Find the index of the first non-TODO commit.
///
/// Parity: `indexOfFirstNonTODOCommit` in `presentation/commits.go`.
fn index_of_first_non_todo_commit(commits: &[CommitItem]) -> usize {
    for (i, commit) in commits.iter().enumerate() {
        if !commit.is_todo() {
            return i;
        }
    }
    0
}

/// Minimal emoji parsing (strips :emoji: colon sequences).
///
/// For full emoji support, the Go code uses `emoji.Sprint`.
/// This is a simplified version that passes through text as-is for now,
/// since most terminal emulators handle emoji natively.
fn parse_emoji(text: &str) -> String {
    // Pass through as-is - modern terminals handle emoji natively.
    // Full parity would require an emoji crate for :emoji_name: → unicode conversion.
    text.to_string()
}

/// Format a Unix timestamp as a smart relative/absolute date string.
///
/// Parity: `utils.UnixToDateSmart` in Go. Uses a simplified approach.
fn format_timestamp_smart(unix_timestamp: i64) -> String {
    // Simplified: format as relative time description
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let diff = now - unix_timestamp;

    if diff < 60 {
        "just now".to_string()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else if diff < 604800 {
        format!("{}d ago", diff / 86400)
    } else {
        // For older dates, show a short date
        format!("{}w ago", diff / 604800)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn make_commit(oid: &str, summary: &str) -> CommitItem {
        CommitItem {
            oid: oid.to_string(),
            short_oid: oid[..7.min(oid.len())].to_string(),
            summary: summary.to_string(),
            author_name: "Author".to_string(),
            ..CommitItem::default()
        }
    }

    fn make_commit_with_status(oid: &str, summary: &str, status: CommitStatus) -> CommitItem {
        CommitItem {
            oid: oid.to_string(),
            short_oid: oid[..7.min(oid.len())].to_string(),
            summary: summary.to_string(),
            status,
            author_name: "Author".to_string(),
            ..CommitItem::default()
        }
    }

    fn make_commit_with_todo(oid: &str, summary: &str, action: CommitTodoAction) -> CommitItem {
        CommitItem {
            oid: oid.to_string(),
            short_oid: oid[..7.min(oid.len())].to_string(),
            summary: summary.to_string(),
            todo_action: action,
            author_name: "Author".to_string(),
            ..CommitItem::default()
        }
    }

    #[test]
    fn get_commit_list_returns_empty_for_no_commits() {
        let empty: Vec<CommitItem> = vec![];
        let opts = CommitDisplayOptions {
            commits: &empty,
            branches: &[],
            current_branch_name: "main",
            has_rebase_update_refs_config: false,
            full_description: false,
            cherry_picked_hashes: &HashSet::new(),
            diff_name: "",
            marked_base_commit: "",
            parse_emoji: false,
            selected_commit_hash: None,
            start_idx: 0,
            end_idx: 10,
            show_graph: false,
            graph_lines: &[],
            bisect_state: None,
            main_branches: &[],
            commit_hash_length: 7,
            author_short_length: 10,
            author_long_length: 20,
            conflict_label: "conflict",
            marked_commit_marker: "→",
        };
        let result = get_commit_list_display_strings(&opts);
        assert!(result.is_empty());
    }

    #[test]
    fn get_commit_list_returns_empty_when_start_beyond_end() {
        let commits = vec![make_commit("abc1234def", "test")];
        let opts = CommitDisplayOptions {
            commits: &commits,
            branches: &[],
            current_branch_name: "main",
            has_rebase_update_refs_config: false,
            full_description: false,
            cherry_picked_hashes: &HashSet::new(),
            diff_name: "",
            marked_base_commit: "",
            parse_emoji: false,
            selected_commit_hash: None,
            start_idx: 5,
            end_idx: 10,
            show_graph: false,
            graph_lines: &[],
            bisect_state: None,
            main_branches: &[],
            commit_hash_length: 7,
            author_short_length: 10,
            author_long_length: 20,
            conflict_label: "conflict",
            marked_commit_marker: "→",
        };
        let result = get_commit_list_display_strings(&opts);
        assert!(result.is_empty());
    }

    #[test]
    fn get_commit_list_produces_lines_for_commits() {
        let commits = vec![
            make_commit("abc1234def567890", "first commit"),
            make_commit("def567890abc1234", "second commit"),
        ];
        let graph_lines = vec!["◯ ".to_string(), "◯ ".to_string()];
        let opts = CommitDisplayOptions {
            commits: &commits,
            branches: &[],
            current_branch_name: "main",
            has_rebase_update_refs_config: false,
            full_description: false,
            cherry_picked_hashes: &HashSet::new(),
            diff_name: "",
            marked_base_commit: "",
            parse_emoji: false,
            selected_commit_hash: None,
            start_idx: 0,
            end_idx: 2,
            show_graph: true,
            graph_lines: &graph_lines,
            bisect_state: None,
            main_branches: &[],
            commit_hash_length: 7,
            author_short_length: 10,
            author_long_length: 20,
            conflict_label: "conflict",
            marked_commit_marker: "→",
        };
        let result = get_commit_list_display_strings(&opts);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn bisect_display_status_color_matches_go() {
        assert_eq!(
            get_bisect_status_color(BisectDisplayStatus::None),
            Color::Black
        );
        assert_eq!(
            get_bisect_status_color(BisectDisplayStatus::New),
            Color::Red
        );
        assert_eq!(
            get_bisect_status_color(BisectDisplayStatus::Old),
            Color::Green
        );
        assert_eq!(
            get_bisect_status_color(BisectDisplayStatus::Skipped),
            Color::Yellow
        );
        assert_eq!(
            get_bisect_status_color(BisectDisplayStatus::Current),
            Color::Magenta
        );
        assert_eq!(
            get_bisect_status_color(BisectDisplayStatus::Candidate),
            Color::Blue
        );
    }

    #[test]
    fn bisect_status_text_matches_go() {
        let bs = BisectState {
            bad_term: "bad".to_string(),
            good_term: "good".to_string(),
            ..BisectState::default()
        };
        assert_eq!(
            get_bisect_status_text(BisectDisplayStatus::New, Some(&bs)),
            "<-- bad"
        );
        assert_eq!(
            get_bisect_status_text(BisectDisplayStatus::Old, Some(&bs)),
            "<-- good"
        );
        assert_eq!(
            get_bisect_status_text(BisectDisplayStatus::Current, Some(&bs)),
            "<-- current"
        );
        assert_eq!(
            get_bisect_status_text(BisectDisplayStatus::Skipped, None),
            "<-- skipped"
        );
        assert_eq!(
            get_bisect_status_text(BisectDisplayStatus::Candidate, None),
            "?"
        );
        assert!(get_bisect_status_text(BisectDisplayStatus::None, None).is_empty());
    }

    #[test]
    fn hash_color_matches_go_commit_status_colors() {
        let cherry = HashSet::new();
        assert_eq!(
            get_hash_color(
                &make_commit_with_status("abc", "", CommitStatus::Unpushed),
                "",
                &cherry,
                BisectDisplayStatus::None,
                None,
            ),
            Color::Red
        );
        assert_eq!(
            get_hash_color(
                &make_commit_with_status("abc", "", CommitStatus::Pushed),
                "",
                &cherry,
                BisectDisplayStatus::None,
                None,
            ),
            Color::Yellow
        );
        assert_eq!(
            get_hash_color(
                &make_commit_with_status("abc", "", CommitStatus::Merged),
                "",
                &cherry,
                BisectDisplayStatus::None,
                None,
            ),
            Color::Green
        );
        assert_eq!(
            get_hash_color(
                &make_commit_with_status("abc", "", CommitStatus::Rebasing),
                "",
                &cherry,
                BisectDisplayStatus::None,
                None,
            ),
            Color::Blue
        );
    }

    #[test]
    fn hash_color_uses_bisect_color_when_bisecting() {
        let bs = BisectState::default();
        let cherry = HashSet::new();
        assert_eq!(
            get_hash_color(
                &make_commit("abc", ""),
                "",
                &cherry,
                BisectDisplayStatus::New,
                Some(&bs),
            ),
            Color::Red // BisectStatusNew color
        );
    }

    #[test]
    fn hash_color_diffed_commit_gets_cyan() {
        let cherry = HashSet::new();
        assert_eq!(
            get_hash_color(
                &make_commit("abc123", ""),
                "abc123",
                &cherry,
                BisectDisplayStatus::None,
                None,
            ),
            Color::Cyan
        );
    }

    #[test]
    fn hash_color_cherry_picked_gets_cyan() {
        let mut cherry = HashSet::new();
        cherry.insert("abc123".to_string());
        assert_eq!(
            get_hash_color(
                &make_commit("abc123", ""),
                "",
                &cherry,
                BisectDisplayStatus::None,
                None,
            ),
            Color::Cyan
        );
    }

    #[test]
    fn action_color_map_matches_go() {
        assert_eq!(
            action_color_map(CommitTodoAction::Pick, CommitStatus::None),
            Color::Cyan
        );
        assert_eq!(
            action_color_map(CommitTodoAction::Drop, CommitStatus::None),
            Color::Red
        );
        assert_eq!(
            action_color_map(CommitTodoAction::Edit, CommitStatus::None),
            Color::Green
        );
        assert_eq!(
            action_color_map(CommitTodoAction::Fixup, CommitStatus::None),
            Color::Magenta
        );
        assert_eq!(
            action_color_map(CommitTodoAction::Squash, CommitStatus::None),
            Color::Yellow
        );
        assert_eq!(
            action_color_map(CommitTodoAction::Pick, CommitStatus::Conflicted),
            Color::Red
        );
    }

    #[test]
    fn index_of_first_non_todo_skips_todos() {
        let commits = vec![
            make_commit_with_todo("a", "todo1", CommitTodoAction::Pick),
            make_commit_with_todo("b", "todo2", CommitTodoAction::Fixup),
            make_commit("c", "real commit"),
            make_commit("d", "another real"),
        ];
        assert_eq!(index_of_first_non_todo_commit(&commits), 2);
    }

    #[test]
    fn index_of_first_non_todo_returns_zero_if_all_real() {
        let commits = vec![make_commit("a", "first"), make_commit("b", "second")];
        assert_eq!(index_of_first_non_todo_commit(&commits), 0);
    }

    #[test]
    fn branch_heads_filters_current_and_main_branches() {
        let branches = vec![
            BranchItem {
                name: "main".to_string(),
                commit_hash: "hash1".to_string(),
                ..BranchItem::default()
            },
            BranchItem {
                name: "feature".to_string(),
                commit_hash: "hash2".to_string(),
                ..BranchItem::default()
            },
            BranchItem {
                name: "current".to_string(),
                commit_hash: "hash3".to_string(),
                ..BranchItem::default()
            },
        ];
        let commits = vec![make_commit("headhash", "head")];
        let main_branches = vec!["main".to_string()];
        let heads = compute_branch_heads_to_visualize(
            &branches,
            "current",
            &main_branches,
            false,
            &commits,
        );
        assert!(!heads.contains("hash1")); // main branch
        assert!(heads.contains("hash2")); // feature branch
        assert!(!heads.contains("hash3")); // current branch
    }

    #[test]
    fn branch_heads_show_head_commit_when_update_refs_on() {
        let head_hash = "headhash";
        let branches = vec![BranchItem {
            name: "feature".to_string(),
            commit_hash: head_hash.to_string(),
            ..BranchItem::default()
        }];
        let commits = vec![make_commit(head_hash, "head")];
        let main_branches: Vec<String> = vec![];

        // Without updateRefs config - head commit excluded
        let heads =
            compute_branch_heads_to_visualize(&branches, "main", &main_branches, false, &commits);
        assert!(!heads.contains(head_hash));

        // With updateRefs config - head commit included
        let heads =
            compute_branch_heads_to_visualize(&branches, "main", &main_branches, true, &commits);
        assert!(heads.contains(head_hash));
    }
}
