use std::collections::HashSet;

const START_COMMIT_HASH: &str = "START";
const EMPTY_TREE_COMMIT_HASH: &str = "EMPTY_TREE";
const COMMIT_SYMBOL: char = '◯';
const MERGE_SYMBOL: char = '⏣';

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GraphCommit {
    pub(crate) oid: String,
    pub(crate) parents: Vec<String>,
}

impl GraphCommit {
    fn primary_parent(&self) -> &str {
        self.parents
            .first()
            .map(String::as_str)
            .unwrap_or(EMPTY_TREE_COMMIT_HASH)
    }

    fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum PipeKind {
    Terminates,
    Starts,
    Continues,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Pipe {
    from_hash: String,
    to_hash: String,
    from_pos: i16,
    to_pos: i16,
    kind: PipeKind,
}

impl Pipe {
    fn left(&self) -> i16 {
        self.from_pos.min(self.to_pos)
    }

    fn right(&self) -> i16 {
        self.from_pos.max(self.to_pos)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum CellType {
    #[default]
    Connection,
    Commit,
    Merge,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Cell {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    cell_type: CellType,
}

impl Cell {
    fn render(&self, writer: &mut String) {
        let (first, second) = get_box_drawing_chars(self.up, self.down, self.left, self.right);
        let adjusted_first = match self.cell_type {
            CellType::Connection => first,
            CellType::Commit => COMMIT_SYMBOL.to_string(),
            CellType::Merge => MERGE_SYMBOL.to_string(),
        };
        writer.push_str(&adjusted_first);
        writer.push_str(second);
    }

    fn set_up(&mut self) {
        self.up = true;
    }

    fn set_down(&mut self) {
        self.down = true;
    }

    fn set_left(&mut self) {
        self.left = true;
    }

    fn set_right(&mut self) {
        self.right = true;
    }

    fn set_type(&mut self, cell_type: CellType) {
        self.cell_type = cell_type;
    }
}

pub(crate) fn render_commit_graph(commits: &[GraphCommit]) -> Vec<String> {
    if commits.is_empty() {
        return Vec::new();
    }

    let pipe_sets = get_pipe_sets(commits);
    pipe_sets
        .iter()
        .map(|pipes| render_pipe_set(pipes).trim_end().to_string())
        .collect()
}

fn get_pipe_sets(commits: &[GraphCommit]) -> Vec<Vec<Pipe>> {
    if commits.is_empty() {
        return Vec::new();
    }

    let mut pipes = vec![Pipe {
        from_hash: START_COMMIT_HASH.to_string(),
        to_hash: commits[0].oid.clone(),
        from_pos: 0,
        to_pos: 0,
        kind: PipeKind::Starts,
    }];

    commits
        .iter()
        .map(|commit| {
            pipes = get_next_pipes(&pipes, commit);
            pipes.clone()
        })
        .collect()
}

fn get_next_pipes(prev_pipes: &[Pipe], commit: &GraphCommit) -> Vec<Pipe> {
    let max_pos = prev_pipes.iter().map(|pipe| pipe.to_pos).max().unwrap_or(0);
    let current_pipes = prev_pipes
        .iter()
        .filter(|pipe| pipe.kind != PipeKind::Terminates)
        .cloned()
        .collect::<Vec<_>>();

    let mut new_pipes = Vec::with_capacity(current_pipes.len() + commit.parents.len().max(1));
    let mut pos = max_pos + 1;
    for pipe in &current_pipes {
        if pipe.to_hash == commit.oid {
            pos = pipe.to_pos;
            break;
        }
    }

    let mut taken_spots = HashSet::new();
    let mut traversed_spots = HashSet::new();
    new_pipes.push(Pipe {
        from_hash: commit.oid.clone(),
        to_hash: commit.primary_parent().to_string(),
        from_pos: pos,
        to_pos: pos,
        kind: PipeKind::Starts,
    });

    let traversed_spots_for_continuing = current_pipes
        .iter()
        .filter(|pipe| pipe.to_hash != commit.oid)
        .map(|pipe| pipe.to_pos)
        .collect::<HashSet<_>>();

    for pipe in &current_pipes {
        if pipe.to_hash == commit.oid {
            new_pipes.push(Pipe {
                from_hash: pipe.from_hash.clone(),
                to_hash: pipe.to_hash.clone(),
                from_pos: pipe.to_pos,
                to_pos: pos,
                kind: PipeKind::Terminates,
            });
            traverse_spots(&mut traversed_spots, &mut taken_spots, pipe.to_pos, pos);
        } else if pipe.to_pos < pos {
            let available_pos = next_available_position(&traversed_spots, None);
            new_pipes.push(Pipe {
                from_hash: pipe.from_hash.clone(),
                to_hash: pipe.to_hash.clone(),
                from_pos: pipe.to_pos,
                to_pos: available_pos,
                kind: PipeKind::Continues,
            });
            traverse_spots(
                &mut traversed_spots,
                &mut taken_spots,
                pipe.to_pos,
                available_pos,
            );
        }
    }

    if commit.is_merge() {
        for parent in commit.parents.iter().skip(1) {
            let available_pos =
                next_available_position(&taken_spots, Some(&traversed_spots_for_continuing));
            new_pipes.push(Pipe {
                from_hash: commit.oid.clone(),
                to_hash: parent.clone(),
                from_pos: pos,
                to_pos: available_pos,
                kind: PipeKind::Starts,
            });
            taken_spots.insert(available_pos);
        }
    }

    for pipe in &current_pipes {
        if pipe.to_hash != commit.oid && pipe.to_pos > pos {
            let mut last = pipe.to_pos;
            let mut position = pipe.to_pos;
            while position > pos {
                if taken_spots.contains(&position) || traversed_spots.contains(&position) {
                    break;
                }
                last = position;
                position -= 1;
            }
            new_pipes.push(Pipe {
                from_hash: pipe.from_hash.clone(),
                to_hash: pipe.to_hash.clone(),
                from_pos: pipe.to_pos,
                to_pos: last,
                kind: PipeKind::Continues,
            });
            traverse_spots(&mut traversed_spots, &mut taken_spots, pipe.to_pos, last);
        }
    }

    new_pipes.sort_by(|left, right| {
        left.to_pos
            .cmp(&right.to_pos)
            .then(left.kind.cmp(&right.kind))
    });
    new_pipes
}

fn next_available_position(occupied: &HashSet<i16>, blocked: Option<&HashSet<i16>>) -> i16 {
    let mut candidate = 0;
    loop {
        if !occupied.contains(&candidate)
            && blocked.is_none_or(|blocked_positions| !blocked_positions.contains(&candidate))
        {
            return candidate;
        }
        candidate += 1;
    }
}

fn traverse_spots(
    traversed_spots: &mut HashSet<i16>,
    taken_spots: &mut HashSet<i16>,
    from: i16,
    to: i16,
) {
    let (left, right) = if from <= to { (from, to) } else { (to, from) };
    for position in left..=right {
        traversed_spots.insert(position);
    }
    taken_spots.insert(to);
}

fn render_pipe_set(pipes: &[Pipe]) -> String {
    let mut max_pos = 0;
    let mut commit_pos = 0;
    let mut start_count = 0;
    for pipe in pipes {
        match pipe.kind {
            PipeKind::Starts => {
                start_count += 1;
                commit_pos = pipe.from_pos;
            }
            PipeKind::Terminates => {
                commit_pos = pipe.to_pos;
            }
            PipeKind::Continues => {}
        }
        max_pos = max_pos.max(pipe.right());
    }

    let is_merge = start_count > 1;
    let mut cells = vec![Cell::default(); max_pos as usize + 1];

    let render_pipe = |cells: &mut [Cell], pipe: &Pipe| {
        let left = pipe.left();
        let right = pipe.right();

        if left != right {
            for position in left + 1..right {
                cells[position as usize].set_left();
                cells[position as usize].set_right();
            }
            cells[left as usize].set_right();
            cells[right as usize].set_left();
        }

        if matches!(pipe.kind, PipeKind::Starts | PipeKind::Continues) {
            cells[pipe.to_pos as usize].set_down();
        }
        if matches!(pipe.kind, PipeKind::Terminates | PipeKind::Continues) {
            cells[pipe.from_pos as usize].set_up();
        }
    };

    for pipe in pipes.iter().filter(|pipe| pipe.kind == PipeKind::Starts) {
        render_pipe(&mut cells, pipe);
    }
    for pipe in pipes.iter().filter(|pipe| {
        pipe.kind != PipeKind::Starts
            && !(pipe.kind == PipeKind::Terminates
                && pipe.from_pos == commit_pos
                && pipe.to_pos == commit_pos)
    }) {
        render_pipe(&mut cells, pipe);
    }

    if is_merge {
        cells[commit_pos as usize].set_type(CellType::Merge);
    } else {
        cells[commit_pos as usize].set_type(CellType::Commit);
    }

    let mut writer = String::with_capacity(cells.len() * 2);
    for cell in cells {
        cell.render(&mut writer);
    }
    writer
}

fn get_box_drawing_chars(up: bool, down: bool, left: bool, right: bool) -> (String, &'static str) {
    match (up, down, left, right) {
        (true, true, true, true) => ("│".to_string(), "─"),
        (true, true, true, false) => ("│".to_string(), " "),
        (true, true, false, true) => ("│".to_string(), "─"),
        (true, true, false, false) => ("│".to_string(), " "),
        (true, false, true, true) => ("┴".to_string(), "─"),
        (true, false, true, false) => ("╯".to_string(), " "),
        (true, false, false, true) => ("╰".to_string(), "─"),
        (true, false, false, false) => ("╵".to_string(), " "),
        (false, true, true, true) => ("┬".to_string(), "─"),
        (false, true, true, false) => ("╮".to_string(), " "),
        (false, true, false, true) => ("╭".to_string(), "─"),
        (false, true, false, false) => ("╷".to_string(), " "),
        (false, false, true, true) => ("─".to_string(), "─"),
        (false, false, true, false) => ("─".to_string(), " "),
        (false, false, false, true) => ("╶".to_string(), "─"),
        (false, false, false, false) => (" ".to_string(), " "),
    }
}

#[cfg(test)]
mod tests {
    use super::{get_box_drawing_chars, render_commit_graph, GraphCommit};

    fn commit(hash: &str, parents: &[&str]) -> GraphCommit {
        GraphCommit {
            oid: hash.to_string(),
            parents: parents.iter().map(|parent| (*parent).to_string()).collect(),
        }
    }

    fn render_rows(commits: &[(&str, &[&str])]) -> Vec<String> {
        let commits = commits
            .iter()
            .map(|(hash, parents)| commit(hash, parents))
            .collect::<Vec<_>>();
        render_commit_graph(&commits)
    }

    #[test]
    fn box_drawing_chars_cover_soft_line_glyphs() {
        assert_eq!(
            get_box_drawing_chars(true, true, false, false),
            ("│".to_string(), " ")
        );
        assert_eq!(
            get_box_drawing_chars(false, true, false, true),
            ("╭".to_string(), "─")
        );
        assert_eq!(
            get_box_drawing_chars(true, false, false, true),
            ("╰".to_string(), "─")
        );
        assert_eq!(
            get_box_drawing_chars(false, true, true, true),
            ("┬".to_string(), "─")
        );
        assert_eq!(
            get_box_drawing_chars(true, false, true, true),
            ("┴".to_string(), "─")
        );
    }

    #[test]
    fn render_commit_graph_handles_linear_history() {
        let rows = render_rows(&[("1", &["2"]), ("2", &["3"]), ("3", &[])]);

        assert_eq!(rows, vec!["◯", "◯", "◯"]);
    }

    #[test]
    fn render_commit_graph_handles_merges() {
        let rows = render_rows(&[
            ("1", &["2"]),
            ("2", &["3"]),
            ("3", &["4"]),
            ("4", &["5", "7"]),
            ("7", &["5"]),
            ("5", &["8"]),
            ("8", &["9"]),
            ("9", &["A", "B"]),
            ("B", &["D"]),
            ("D", &["D"]),
            ("A", &["E"]),
            ("E", &["F"]),
            ("F", &["D"]),
            ("D", &["G"]),
        ]);

        assert_eq!(
            rows,
            vec![
                "◯",
                "◯",
                "◯",
                "⏣─╮",
                "│ ◯",
                "◯─╯",
                "◯",
                "⏣─╮",
                "│ ◯",
                "│ ◯",
                "◯ │",
                "◯ │",
                "◯ │",
                "◯─╯",
            ]
        );
    }

    #[test]
    fn render_commit_graph_moves_paths_left_when_space_opens() {
        let rows = render_rows(&[
            ("1", &["2"]),
            ("2", &["3", "4"]),
            ("4", &["3", "5"]),
            ("3", &["5"]),
            ("5", &["6"]),
            ("6", &["7"]),
        ]);

        assert_eq!(rows, vec!["◯", "⏣─╮", "│ ⏣─╮", "◯─╯ │", "◯───╯", "◯"]);
    }

    #[test]
    fn render_commit_graph_handles_new_commit_in_all_branches_view() {
        let rows = render_rows(&[
            ("1", &["2"]),
            ("2", &["3", "4"]),
            ("4", &["3", "5"]),
            ("Z", &["Z"]),
            ("3", &["5"]),
            ("5", &["6"]),
            ("6", &["7"]),
        ]);

        assert_eq!(
            rows,
            vec![
                "◯",
                "⏣─╮",
                "│ ⏣─╮",
                "│ │ │ ◯",
                "◯─╯ │ │",
                "◯───╯ │",
                "◯ ╭───╯"
            ]
        );
    }

    #[test]
    fn render_commit_graph_handles_multi_branch_continuation() {
        let rows = render_rows(&[
            ("1", &["2"]),
            ("2", &["3", "4"]),
            ("3", &["5", "4"]),
            ("5", &["7", "8"]),
            ("7", &["4", "A"]),
            ("4", &["B"]),
            ("B", &["C"]),
            ("C", &["D"]),
        ]);

        assert_eq!(
            rows,
            vec![
                "◯",
                "⏣─╮",
                "⏣─│─╮",
                "⏣─│─│─╮",
                "⏣─│─│─│─╮",
                "◯─┴─╯ │ │",
                "◯ ╭───╯ │",
                "◯ │ ╭───╯",
            ]
        );
    }

    #[test]
    fn render_commit_graph_fills_gaps_before_continuing_right_side_paths() {
        let rows = render_rows(&[
            ("1", &["2", "3", "4", "5"]),
            ("4", &["2"]),
            ("2", &["A"]),
            ("A", &["6", "B"]),
            ("B", &["C"]),
        ]);

        assert_eq!(
            rows,
            vec!["⏣─┬─┬─╮", "│ │ ◯ │", "◯─│─╯ │", "⏣─│─╮ │", "│ │ ◯ │"]
        );
    }

    #[test]
    fn render_commit_graph_left_move_continues_with_merge_parent() {
        // Go test: "with a path that has room to move to the left and continues"
        // commits: 1→2, 2→{3,4}, 3→{5,4}, 5→{7,8}, 4→7, 7→11
        let rows = render_rows(&[
            ("1", &["2"]),
            ("2", &["3", "4"]),
            ("3", &["5", "4"]),
            ("5", &["7", "8"]),
            ("4", &["7"]),
            ("7", &["11"]),
        ]);

        assert_eq!(
            rows,
            vec!["◯", "⏣─╮", "⏣─│─╮", "⏣─│─│─╮", "│ ◯─╯ │", "◯─╯ ╭─╯"]
        );
    }

    #[test]
    fn render_commit_graph_terminates_with_merge_into_same_pos() {
        // Go test: "with a path that has room to move to the left and continues"
        // commits: 1→{2,3}, 3→2, 2→{4,5}, 4→{6,7}, 6→8
        let rows = render_rows(&[
            ("1", &["2", "3"]),
            ("3", &["2"]),
            ("2", &["4", "5"]),
            ("4", &["6", "7"]),
            ("6", &["8"]),
        ]);

        assert_eq!(rows, vec!["⏣─╮", "│ ◯", "⏣─│", "⏣─│─╮", "◯ │ │"]);
    }

    #[test]
    fn render_commit_graph_deep_chain_left_continue() {
        // Go test: full 8-commit chain with deep left-moving continuations
        let rows = render_rows(&[
            ("1", &["2"]),
            ("2", &["3", "4"]),
            ("3", &["5", "4"]),
            ("5", &["7", "8"]),
            ("7", &["4", "A"]),
            ("4", &["B"]),
            ("B", &["C"]),
            ("C", &["D"]),
        ]);

        assert_eq!(
            rows,
            vec![
                "◯",
                "⏣─╮",
                "⏣─│─╮",
                "⏣─│─│─╮",
                "⏣─│─│─│─╮",
                "◯─┴─╯ │ │",
                "◯ ╭───╯ │",
                "◯ │ ╭───╯",
            ]
        );
    }

    #[test]
    fn render_commit_graph_deeper_chain_with_more_merges() {
        // Go test: 10-commit chain with multiple merge parents and deep continuations
        let rows = render_rows(&[
            ("1", &["2"]),
            ("2", &["3", "4"]),
            ("3", &["5", "4"]),
            ("5", &["7", "G"]),
            ("7", &["8", "A"]),
            ("8", &["4", "E"]),
            ("4", &["B"]),
            ("B", &["C"]),
            ("C", &["D"]),
            ("D", &["F"]),
        ]);

        assert_eq!(
            rows,
            vec![
                "◯",
                "⏣─╮",
                "⏣─│─╮",
                "⏣─│─│─╮",
                "⏣─│─│─│─╮",
                "⏣─│─│─│─│─╮",
                "◯─┴─╯ │ │ │",
                "◯ ╭───╯ │ │",
                "◯ │ ╭───╯ │",
                "◯ │ │ ╭───╯",
            ]
        );
    }
}
