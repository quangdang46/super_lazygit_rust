use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::RwLock;

use crate::style::text_style::TextStyle;

use super::cell::{Cell, CellType};

const EMPTY_TREE_COMMIT_HASH: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
const START_COMMIT_HASH: &str = "START";

#[derive(Clone, Copy, PartialEq, Eq)]
enum PipeKind {
    Terminates,
    Starts,
    Continues,
}

#[derive(Clone)]
pub struct Pipe {
    from_hash: Option<String>,
    to_hash: Option<String>,
    style: TextStyle,
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

static HIGHLIGHT_STYLE: TextStyle = TextStyle::new();

struct RgbCacheKey {
    r: u8,
    g: u8,
    b: u8,
    str: String,
}

impl PartialEq for RgbCacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.r == other.r && self.g == other.g && self.b == other.b && self.str == other.str
    }
}

impl Eq for RgbCacheKey {}

impl std::hash::Hash for RgbCacheKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.r.hash(state);
        self.g.hash(state);
        self.b.hash(state);
        self.str.hash(state);
    }
}

static RGB_CACHE: RwLock<std::collections::HashMap<RgbCacheKey, String>> =
    RwLock::new(std::collections::HashMap::new());

fn cached_sprint(style: TextStyle, str: &str) -> String {
    let color = style.get_fg_color();
    if let Some((r, g, b)) = color {
        let key = RgbCacheKey {
            r,
            g,
            b,
            str: str.to_string(),
        };
        {
            let cache = RGB_CACHE.read().unwrap();
            if let Some(value) = cache.get(&key) {
                return value.clone();
            }
        }
        let value = style.sprint(str);
        {
            let mut cache = RGB_CACHE.write().unwrap();
            cache.insert(key, value.clone());
        }
        value
    } else {
        style.sprint(str)
    }
}

pub fn render_commit_graph(
    commits: &[CommitInfo],
    selected_commit_hash: Option<&str>,
    get_style: impl Fn(&CommitInfo) -> TextStyle,
) -> Vec<String> {
    let pipe_sets = get_pipe_sets(commits, &get_style);
    if pipe_sets.is_empty() {
        return Vec::new();
    }

    render_aux(pipe_sets, commits, selected_commit_hash)
}

struct CommitInfo {
    hash: String,
    parent_hashes: Vec<String>,
    is_merge: bool,
    is_first: bool,
}

impl CommitInfo {
    fn hash_ptr(&self) -> Option<&str> {
        Some(&self.hash)
    }

    fn parent_ptrs(&self) -> Vec<Option<&str>> {
        self.parent_hashes
            .iter()
            .map(|s| Some(s.as_str()))
            .collect()
    }

    fn is_first_commit(&self) -> bool {
        self.is_first
    }
}

pub fn get_pipe_sets(
    commits: &[CommitInfo],
    get_style: impl Fn(&CommitInfo) -> TextStyle,
) -> Vec<Vec<Pipe>> {
    if commits.is_empty() {
        return Vec::new();
    }

    let mut pipe_sets: Vec<Vec<Pipe>> = Vec::new();
    let mut current_pipes: Vec<Pipe> = Vec::new();

    current_pipes.push(Pipe {
        from_hash: Some(START_COMMIT_HASH.to_string()),
        to_hash: commits.first().map(|c| c.hash.clone()),
        style: TextStyle::new(),
        from_pos: 0,
        to_pos: 0,
        kind: PipeKind::Starts,
    });
    pipe_sets.push(current_pipes.clone());

    for commit in commits {
        current_pipes = get_next_pipes(&current_pipes, commit, &get_style);
        pipe_sets.push(current_pipes.clone());
    }

    pipe_sets
}

fn get_next_pipes(
    prev_pipes: &[Pipe],
    commit: &CommitInfo,
    get_style: impl Fn(&CommitInfo) -> TextStyle,
) -> Vec<Pipe> {
    let mut max_pos: i16 = 0;
    for pipe in prev_pipes {
        if pipe.to_pos > max_pos {
            max_pos = pipe.to_pos;
        }
    }

    let current_pipes: Vec<Pipe> = prev_pipes
        .iter()
        .filter(|pipe| pipe.kind != PipeKind::Terminates)
        .cloned()
        .collect();

    let mut new_pipes: Vec<Pipe> =
        Vec::with_capacity(current_pipes.len() + commit.parent_hashes.len());

    let mut pos = max_pos + 1;
    for pipe in &current_pipes {
        if pipe.to_hash.as_deref() == commit.hash_ptr() {
            pos = pipe.to_pos;
            break;
        }
    }

    let mut taken_spots: HashSet<i16> = HashSet::new();
    let mut traversed_spots: HashSet<i16> = HashSet::new();

    let to_hash = if commit.is_first_commit() {
        Some(EMPTY_TREE_COMMIT_HASH.to_string())
    } else {
        commit.parent_hashes.first().cloned()
    };

    new_pipes.push(Pipe {
        from_hash: Some(commit.hash.clone()),
        to_hash,
        style: get_style(commit),
        from_pos: pos,
        to_pos: pos,
        kind: PipeKind::Starts,
    });

    let mut traversed_spots_for_continuing_pipes: HashSet<i16> = HashSet::new();
    for pipe in &current_pipes {
        if pipe.to_hash.as_deref() != commit.hash_ptr() {
            traversed_spots_for_continuing_pipes.insert(pipe.to_pos);
        }
    }

    let get_next_available_pos_for_continuing_pipe = || -> i16 {
        let mut i: i16 = 0;
        loop {
            if !traversed_spots.contains(&i) {
                return i;
            }
            i += 1;
        }
    };

    let get_next_available_pos_for_new_pipe = || -> i16 {
        let mut i: i16 = 0;
        loop {
            if !taken_spots.contains(&i) && !traversed_spots_for_continuing_pipes.contains(&i) {
                return i;
            }
            i += 1;
        }
    };

    let traverse = |from: i16, to: i16| {
        let (left, right) = if from <= to { (from, to) } else { (to, from) };
        for i in left..=right {
            traversed_spots.insert(i);
        }
        taken_spots.insert(to);
    };

    for pipe in &current_pipes {
        if pipe.to_hash.as_deref() == commit.hash_ptr() {
            new_pipes.push(Pipe {
                from_hash: pipe.from_hash.clone(),
                to_hash: pipe.to_hash.clone(),
                style: pipe.style,
                from_pos: pipe.to_pos,
                to_pos: pos,
                kind: PipeKind::Terminates,
            });
            traverse(pipe.to_pos, pos);
        } else if pipe.to_pos < pos {
            let available_pos = get_next_available_pos_for_continuing_pipe();
            new_pipes.push(Pipe {
                from_hash: pipe.from_hash.clone(),
                to_hash: pipe.to_hash.clone(),
                style: pipe.style,
                from_pos: pipe.to_pos,
                to_pos: available_pos,
                kind: PipeKind::Continues,
            });
            traverse(pipe.to_pos, available_pos);
        }
    }

    if commit.is_merge {
        for parent in &commit.parent_hashes[1..] {
            let available_pos = get_next_available_pos_for_new_pipe();
            new_pipes.push(Pipe {
                from_hash: Some(commit.hash.clone()),
                to_hash: Some(parent.clone()),
                style: get_style(commit),
                from_pos: pos,
                to_pos: available_pos,
                kind: PipeKind::Starts,
            });
            taken_spots.insert(available_pos);
        }
    }

    for pipe in &current_pipes {
        if pipe.to_hash.as_deref() != commit.hash_ptr() && pipe.to_pos > pos {
            let mut last = pipe.to_pos;
            for i in (pos + 1..=pipe.to_pos).rev() {
                if taken_spots.contains(&i) || traversed_spots.contains(&i) {
                    break;
                }
                last = i;
            }
            new_pipes.push(Pipe {
                from_hash: pipe.from_hash.clone(),
                to_hash: pipe.to_hash.clone(),
                style: pipe.style,
                from_pos: pipe.to_pos,
                to_pos: last,
                kind: PipeKind::Continues,
            });
            traverse(pipe.to_pos, last);
        }
    }

    new_pipes.sort_by(|a, b| {
        let pos_cmp = a.to_pos.cmp(&b.to_pos);
        if pos_cmp == Ordering::Equal {
            a.kind.cmp(&b.kind)
        } else {
            pos_cmp
        }
    });

    new_pipes
}

fn render_aux(
    pipe_sets: Vec<Vec<Pipe>>,
    commits: &[CommitInfo],
    selected_commit_hash: Option<&str>,
) -> Vec<String> {
    let num_procs = std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(1);
    let per_proc = pipe_sets.len() / num_procs;

    let mut chunks: Vec<Vec<String>> = Vec::with_capacity(num_procs);
    for _ in 0..num_procs {
        chunks.push(Vec::new());
    }

    std::thread::scope(|s| {
        let mut handles = Vec::with_capacity(num_procs);

        for i in 0..num_procs {
            let start = i * per_proc;
            let end = if i == num_procs - 1 {
                pipe_sets.len()
            } else {
                (i + 1) * per_proc
            };

            let pipe_sets_chunk = pipe_sets[start..end].to_vec();
            let commits = commits.to_vec();
            let selected = selected_commit_hash.map(|s| s.to_string());

            handles.push(s.spawn(move || {
                let mut lines = Vec::with_capacity(end - start);
                for (j, pipe_set) in pipe_sets_chunk.iter().enumerate() {
                    let k = start + j;
                    let prev_commit = if k > 0 { commits.get(k - 1) } else { None };
                    let line = render_pipe_set(pipe_set, selected.as_deref(), prev_commit);
                    lines.push(line);
                }
                lines
            }));
        }

        for handle in handles {
            let result = handle.join().unwrap();
            let idx = handles
                .iter()
                .position(|h| h.thread().id() == handle.thread().id())
                .unwrap_or(0);
            chunks[idx] = result;
        }
    });

    chunks.into_iter().flatten().collect()
}

fn render_pipe_set(
    pipes: &[Pipe],
    selected_commit_hash: Option<&str>,
    prev_commit: Option<&CommitInfo>,
) -> String {
    let mut max_pos: i16 = 0;
    let mut commit_pos: i16 = 0;
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
            _ => {}
        }

        if pipe.right() > max_pos {
            max_pos = pipe.right();
        }
    }

    let is_merge = start_count > 1;

    let mut cells: Vec<Cell> = (0..=max_pos as usize).map(|_| Cell::default()).collect();

    for cell in cells.iter_mut() {
        cell.set_type(CellType::Connection);
    }

    let render_pipe = |pipe: &Pipe, style: TextStyle, override_right_style: bool| {
        let left = pipe.left();
        let right = pipe.right();

        if left != right {
            for i in (left + 1)..right {
                cells[i as usize].set_left(style);
                cells[i as usize].set_right(style, override_right_style);
            }
            cells[left as usize].set_right(style, override_right_style);
            cells[right as usize].set_left(style);
        }

        if pipe.kind == PipeKind::Starts || pipe.kind == PipeKind::Continues {
            cells[pipe.to_pos as usize].set_down(style);
        }
        if pipe.kind == PipeKind::Terminates || pipe.kind == PipeKind::Continues {
            cells[pipe.from_pos as usize].set_up(style);
        }
    };

    let mut highlight = true;
    if let Some(prev) = prev_commit {
        if prev.hash_ptr() == selected_commit_hash {
            highlight = false;
            for pipe in pipes {
                if pipe.from_hash.as_deref() == selected_commit_hash
                    && (pipe.kind != PipeKind::Terminates || pipe.from_pos != pipe.to_pos)
                {
                    highlight = true;
                    break;
                }
            }
        }
    }

    let selected_pipes: Vec<&Pipe> = pipes
        .iter()
        .filter(|pipe| highlight && pipe.from_hash.as_deref() == selected_commit_hash)
        .collect();

    let non_selected_pipes: Vec<&Pipe> = pipes
        .iter()
        .filter(|pipe| !(highlight && pipe.from_hash.as_deref() == selected_commit_hash))
        .collect();

    for pipe in &non_selected_pipes {
        if pipe.kind == PipeKind::Starts {
            render_pipe(pipe, pipe.style, true);
        }
    }

    for pipe in &non_selected_pipes {
        if pipe.kind != PipeKind::Starts
            && !(pipe.kind == PipeKind::Terminates
                && pipe.from_pos == commit_pos
                && pipe.to_pos == commit_pos)
        {
            render_pipe(pipe, pipe.style, false);
        }
    }

    for pipe in &selected_pipes {
        for i in pipe.left()..=pipe.right() {
            cells[i as usize].reset();
        }
    }

    let highlight_style = HIGHLIGHT_STYLE;

    for pipe in &selected_pipes {
        render_pipe(pipe, highlight_style, true);
        if pipe.to_pos == commit_pos {
            cells[pipe.to_pos as usize].set_style(highlight_style);
        }
    }

    let c_type = if is_merge {
        CellType::Merge
    } else {
        CellType::Commit
    };
    cells[commit_pos as usize].set_type(c_type);

    let mut writer = String::new();
    for cell in cells {
        cell.render(&mut writer);
    }
    writer
}

fn equal_hashes(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}
