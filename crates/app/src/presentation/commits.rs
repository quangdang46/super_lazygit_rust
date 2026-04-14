use std::collections::HashSet;

use crate::style::basic_styles::{
    fg_black, fg_blue, fg_cyan, fg_green, fg_magenta, fg_red, fg_yellow,
};
use crate::style::text_style::TextStyle;
use crate::style::theme::{
    cherry_picked_commit_text_style, default_text_color, diff_terminal_color,
};
use crate::utils::escape_special_chars;

use super::authors::{author_style, author_with_length};
use super::graph::render_commit_graph;
use super_lazygit_core::state::{
    BisectCommitStatus, BisectState, BranchItem, CommitDivergence, CommitItem, CommitStatus,
    CommitTodoAction,
};

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
    pub bisect_info: Option<&'a BisectState>,
    pub main_branches: &'a [&'a str],
    pub commit_hash_length: usize,
    pub author_short_length: usize,
    pub author_long_length: usize,
    pub conflict_label: &'a str,
    pub marked_commit_marker: &'a str,
    pub new_term: &'a str,
    pub old_term: &'a str,
}

pub fn get_commit_list_display_strings(opts: &CommitDisplayOptions<'_>) -> Vec<Vec<String>> {
    if opts.commits.is_empty() {
        return Vec::new();
    }

    if opts.start_idx >= opts.commits.len() {
        return Vec::new();
    }

    let end_idx = std::cmp::min(opts.end_idx, opts.commits.len());
    let rebase_offset = std::cmp::min(index_of_first_non_todo_commit(opts.commits), end_idx);
    let filtered_commits = &opts.commits[opts.start_idx..end_idx];
    let bisect_bounds = get_bisect_bounds(opts.commits, opts.bisect_info);

    let graph_lines = if opts.show_graph && !opts.commits.is_empty() {
        let graph_offset = std::cmp::max(opts.start_idx, rebase_offset);
        let pipe_sets = load_pipesets(&opts.commits[rebase_offset..]);
        let pipe_set_offset = std::cmp::max(opts.start_idx.saturating_sub(rebase_offset), 0);
        let graph_pipe_sets_end = std::cmp::max(end_idx.saturating_sub(rebase_offset), 0);
        let graph_pipe_sets = if pipe_set_offset < pipe_sets.len() {
            &pipe_sets[pipe_set_offset..graph_pipe_sets_end.max(pipe_set_offset)]
        } else {
            &[]
        };
        let graph_commits_offset = graph_offset;
        let graph_commits_end = end_idx;
        let graph_commits = if graph_commits_offset < opts.commits.len() {
            &opts.commits[graph_commits_offset..graph_commits_end]
        } else {
            &[]
        };
        let get_style = |commit: &CommitItem| author_style(&commit.author_name);
        render_commit_graph(graph_commits, opts.selected_commit_hash, get_style)
    } else {
        Vec::new()
    };

    let get_graph_line = |idx: usize| -> String {
        if !opts.show_graph {
            return String::new();
        }
        let local_idx = idx.saturating_sub(opts.start_idx);
        graph_lines.get(local_idx).cloned().unwrap_or_default()
    };

    let branch_heads_to_visualize: HashSet<String> = opts
        .branches
        .iter()
        .filter_map(|b| {
            if b.commit_hash.is_empty() {
                return None;
            }
            if b.name == opts.current_branch_name {
                return None;
            }
            if opts.main_branches.iter().any(|&mb| mb == b.name.as_str()) {
                return None;
            }
            let head_hash = opts.commits.first().map(|c| c.oid.as_str()).unwrap_or("");
            if !opts.has_rebase_update_refs_config && b.commit_hash == head_hash {
                return None;
            }
            Some(b.commit_hash.clone())
        })
        .collect();

    let mut lines = Vec::with_capacity(filtered_commits.len());
    let mut will_be_rebased = opts.marked_base_commit.is_empty();

    for (i, commit) in filtered_commits.iter().enumerate() {
        let unfiltered_idx = i + opts.start_idx;
        let bisect_status = get_bisect_status(
            unfiltered_idx,
            &commit.oid,
            opts.bisect_info,
            bisect_bounds.as_ref(),
        );
        let is_marked_base_commit = !commit.oid.is_empty() && commit.oid == opts.marked_base_commit;
        if is_marked_base_commit {
            will_be_rebased = true;
        }

        let author_length = if opts.full_description {
            opts.author_long_length
        } else {
            opts.author_short_length
        };

        lines.push(display_commit(
            commit,
            &branch_heads_to_visualize,
            opts.has_rebase_update_refs_config,
            opts.cherry_picked_hashes,
            is_marked_base_commit,
            will_be_rebased,
            opts.diff_name,
            opts.parse_emoji,
            &get_graph_line(unfiltered_idx),
            opts.full_description,
            bisect_status,
            opts.bisect_info,
            author_length,
            opts.commit_hash_length,
            opts.conflict_label,
            opts.marked_commit_marker,
            opts.new_term,
            opts.old_term,
        ));
    }

    lines
}

fn get_bisect_bounds(
    commits: &[CommitItem],
    bisect_info: Option<&BisectState>,
) -> Option<BisectBounds> {
    let bisect_info = bisect_info?;

    if !bisect_info.started() {
        return None;
    }

    let mut bounds = BisectBounds {
        new_index: 0,
        old_index: 0,
    };

    for (i, commit) in commits.iter().enumerate() {
        if commit.oid == bisect_info.get_new_hash() {
            bounds.new_index = i;
        }

        if let Some(status) = bisect_info.status(&commit.oid) {
            if status == BisectCommitStatus::Old {
                bounds.old_index = i;
                return Some(bounds);
            }
        }
    }

    None
}

struct BisectBounds {
    new_index: usize,
    old_index: usize,
}

fn index_of_first_non_todo_commit(commits: &[CommitItem]) -> usize {
    for (i, commit) in commits.iter().enumerate() {
        if !commit.is_todo() {
            return i;
        }
    }
    0
}

fn load_pipesets(commits: &[CommitItem]) -> Vec<Vec<crate::presentation::graph::Pipe>> {
    if commits.is_empty() {
        return Vec::new();
    }

    let get_style = |commit: &CommitItem| author_style(&commit.author_name);
    crate::presentation::graph::get_pipe_sets(commits, get_style)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BisectDisplayStatus {
    None,
    Old,
    New,
    Skipped,
    Candidate,
    Current,
}

fn get_bisect_status(
    index: usize,
    commit_hash: &str,
    bisect_info: Option<&BisectState>,
    bisect_bounds: Option<&BisectBounds>,
) -> BisectDisplayStatus {
    let Some(bisect_info) = bisect_info else {
        return BisectDisplayStatus::None;
    };

    if !bisect_info.started() {
        return BisectDisplayStatus::None;
    }

    if bisect_info.get_current_hash() == Some(commit_hash.to_string()) {
        return BisectDisplayStatus::Current;
    }

    if let Some(status) = bisect_info.status(commit_hash) {
        match status {
            BisectCommitStatus::New => return BisectDisplayStatus::New,
            BisectCommitStatus::Old => return BisectDisplayStatus::Old,
            BisectCommitStatus::Skipped => return BisectDisplayStatus::Skipped,
        }
    }

    if let Some(bounds) = bisect_bounds {
        if index >= bounds.new_index && index <= bounds.old_index {
            return BisectDisplayStatus::Candidate;
        }
        return BisectDisplayStatus::None;
    }

    BisectDisplayStatus::None
}

fn get_bisect_status_text(
    status: BisectDisplayStatus,
    bisect_info: Option<&BisectState>,
    new_term: &str,
    old_term: &str,
) -> String {
    match status {
        BisectDisplayStatus::None => String::new(),
        BisectDisplayStatus::New => {
            let term = bisect_info
                .and_then(|b| b.good_term.as_ref())
                .map(|s| s.as_str())
                .unwrap_or(new_term);
            format!("<-- {term}")
        }
        BisectDisplayStatus::Old => {
            let term = bisect_info
                .and_then(|b| b.bad_term.as_ref())
                .map(|s| s.as_str())
                .unwrap_or(old_term);
            format!("<-- {term}")
        }
        BisectDisplayStatus::Current => "<-- current".to_string(),
        BisectDisplayStatus::Skipped => "<-- skipped".to_string(),
        BisectDisplayStatus::Candidate => "?".to_string(),
    }
}

fn get_bisect_status_color(status: BisectDisplayStatus) -> TextStyle {
    match status {
        BisectDisplayStatus::None => fg_black(),
        BisectDisplayStatus::New => fg_red(),
        BisectDisplayStatus::Old => fg_green(),
        BisectDisplayStatus::Skipped => fg_yellow(),
        BisectDisplayStatus::Current => fg_magenta(),
        BisectDisplayStatus::Candidate => fg_blue(),
    }
}

#[allow(clippy::too_many_arguments)]
fn display_commit(
    commit: &CommitItem,
    branch_heads_to_visualize: &HashSet<String>,
    has_rebase_update_refs_config: bool,
    cherry_picked_commit_hash_set: &HashSet<String>,
    is_marked_base_commit: bool,
    will_be_rebased: bool,
    diff_name: &str,
    parse_emoji: bool,
    graph_line: &str,
    full_description: bool,
    bisect_status: BisectDisplayStatus,
    bisect_info: Option<&BisectState>,
    author_length: usize,
    commit_hash_length: usize,
    conflict_label: &str,
    marked_commit_marker: &str,
    new_term: &str,
    old_term: &str,
) -> Vec<String> {
    let mut cols: Vec<String> = Vec::with_capacity(7);

    let bisect_string = get_bisect_status_text(bisect_status, bisect_info, new_term, old_term);
    let bisect_color = get_bisect_status_color(bisect_status);
    if !bisect_string.is_empty() {
        cols.push(bisect_color.sprint(&bisect_string));
    } else {
        cols.push(String::new());
    }

    let hash_color = get_hash_color(
        commit,
        diff_name,
        cherry_picked_commit_hash_set,
        bisect_status,
        bisect_info,
    );
    let hash_string = if commit_hash_length >= commit.oid.len() {
        hash_color.sprint(&commit.oid)
    } else if commit_hash_length > 0 {
        let end = commit_hash_length.min(commit.oid.len());
        hash_color.sprint(&commit.oid[..end])
    } else {
        hash_color.sprint("*")
    };
    cols.push(hash_string);

    let divergence_string = if commit.divergence != CommitDivergence::None {
        let arrow = match commit.divergence {
            CommitDivergence::Left => "↑",
            CommitDivergence::Right => "↓",
            _ => "",
        };
        hash_color.sprint(arrow)
    } else {
        String::new()
    };
    cols.push(divergence_string);

    let description_string = if full_description && commit.unix_timestamp > 0 {
        fg_blue().sprint(&format_timestamp_smart(commit.unix_timestamp))
    } else {
        String::new()
    };
    cols.push(description_string);

    if commit.todo_action != CommitTodoAction::None {
        let mut action_str = format!("{:?}", commit.todo_action).to_lowercase();
        if !commit.todo_action_flag.is_empty() && commit.todo_action == CommitTodoAction::Fixup {
            action_str = format!("{action_str} {}", commit.todo_action_flag);
        }
        let action_color = action_color_map(commit.todo_action, commit.status);
        cols.push(action_color.sprint(&action_str));
    } else {
        cols.push(String::new());
    }

    let author = author_with_length(&commit.author_name, author_length as i32);
    cols.push(author);

    let mut tail = graph_line.to_string();

    if commit.status == CommitStatus::Conflicted {
        let mark = format!("<-- {conflict_label} ---");
        tail.push_str(&fg_red().sprint(&mark));
        tail.push(' ');
    } else if is_marked_base_commit {
        tail.push_str(&fg_yellow().sprint(marked_commit_marker));
        tail.push(' ');
    } else if !will_be_rebased {
        tail.push_str(&fg_yellow().sprint("✓"));
        tail.push(' ');
    }

    let mut tag_string = String::new();
    if full_description {
        if !commit.extra_info.is_empty() {
            tag_string = format!("{} ", fg_magenta().set_bold().sprint(&commit.extra_info));
        }
    } else {
        if !commit.tags.is_empty() {
            tag_string = format!(
                "{} ",
                diff_terminal_color()
                    .set_bold()
                    .sprint(commit.tags.join(" "))
            );
        }

        if branch_heads_to_visualize.contains(&commit.oid)
            && commit.status != CommitStatus::Merged
            && !(commit.is_todo() && has_rebase_update_refs_config)
        {
            tag_string = format!("{}{} ", fg_cyan().set_bold().sprint("*"), tag_string);
        }
    }
    tail.push_str(&tag_string);

    let mut name = commit.summary.clone();
    if commit.todo_action == CommitTodoAction::UpdateRef {
        name = name.trim_start_matches("refs/heads/").to_string();
    }
    if parse_emoji {
        name = parse_emoji_string(&name);
    }
    tail.push_str(&default_text_color().sprint(&escape_special_chars(&name)));

    cols.push(tail);

    cols
}

fn get_hash_color(
    commit: &CommitItem,
    diff_name: &str,
    cherry_picked_commit_hash_set: &HashSet<String>,
    bisect_status: BisectDisplayStatus,
    bisect_info: Option<&BisectState>,
) -> TextStyle {
    if bisect_info.is_some() {
        return get_bisect_status_color(bisect_status);
    }

    let diffed = !commit.oid.is_empty() && commit.oid == diff_name;
    let hash_color = match commit.status {
        CommitStatus::Unpushed => fg_red(),
        CommitStatus::Pushed => fg_yellow(),
        CommitStatus::Merged => fg_green(),
        CommitStatus::Rebasing
        | CommitStatus::CherryPickingOrReverting
        | CommitStatus::Conflicted => fg_blue(),
        CommitStatus::Reflog => fg_blue(),
        _ => default_text_color(),
    };

    if diffed {
        diff_terminal_color()
    } else if cherry_picked_commit_hash_set.contains(&commit.oid) {
        cherry_picked_commit_text_style(&[], &[])
    } else if commit.divergence == CommitDivergence::Right && commit.status != CommitStatus::Merged
    {
        fg_blue()
    } else {
        hash_color
    }
}

fn action_color_map(action: CommitTodoAction, status: CommitStatus) -> TextStyle {
    if status == CommitStatus::Conflicted {
        return fg_red();
    }

    match action {
        CommitTodoAction::Pick => fg_cyan(),
        CommitTodoAction::Drop => fg_red(),
        CommitTodoAction::Edit => fg_green(),
        CommitTodoAction::Fixup => fg_magenta(),
        _ => fg_yellow(),
    }
}

fn format_timestamp_smart(unix_timestamp: i64) -> String {
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
        format!("{}w ago", diff / 604800)
    }
}

fn parse_emoji_string(s: &str) -> String {
    s.to_string()
}
