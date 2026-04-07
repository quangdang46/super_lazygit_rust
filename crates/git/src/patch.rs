use regex::Regex;

/// Patch line kinds corresponding to Go's PatchLineKind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchLineKind {
    PatchHeader,
    HunkHeader,
    Addition,
    Deletion,
    Context,
    NewlineMessage,
}

/// A single line in a patch
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchLine {
    pub kind: PatchLineKind,
    pub content: String, // something like '+ hello' (note the first character is not removed)
}

impl PatchLine {
    pub fn is_change(&self) -> bool {
        self.kind == PatchLineKind::Addition || self.kind == PatchLineKind::Deletion
    }

    pub fn is_addition(&self) -> bool {
        self.kind == PatchLineKind::Addition
    }

    pub fn is_deletion(&self) -> bool {
        self.kind == PatchLineKind::Deletion
    }
}

/// Returns the number of lines in the given slice that have one of the given kinds
fn n_lines_with_kind(lines: &[PatchLine], kinds: &[PatchLineKind]) -> usize {
    lines
        .iter()
        .filter(|line| kinds.contains(&line.kind))
        .count()
}

/// A hunk in a patch
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    /// the line number of the first line in the old file
    pub old_start: usize,
    /// the line number of the first line in the new file
    pub new_start: usize,
    /// the context at the end of the header line
    pub header_context: String,
    /// the body of the hunk, excluding the header line
    pub body_lines: Vec<PatchLine>,
}

/// Example hunk:
/// @@ -16,2 +14,3 @@ func (f *CommitFile) Description() string {
///     return f.Name
/// -}
/// +
/// +// test

impl Hunk {
    /// Returns the number of lines in the hunk in the original file
    pub fn old_length(&self) -> usize {
        n_lines_with_kind(&self.body_lines, &[PatchLineKind::Context, PatchLineKind::Deletion])
    }

    /// Returns the number of lines in the hunk in the new file
    pub fn new_length(&self) -> usize {
        n_lines_with_kind(&self.body_lines, &[PatchLineKind::Context, PatchLineKind::Addition])
    }

    /// Returns true if the hunk contains any changes
    pub fn contains_changes(&self) -> bool {
        n_lines_with_kind(&self.body_lines, &[PatchLineKind::Addition, PatchLineKind::Deletion]) > 0
    }

    /// Returns the number of lines in the hunk, including the header line
    pub fn line_count(&self) -> usize {
        self.body_lines.len() + 1
    }

    /// Returns all lines in the hunk, including the header line
    pub fn all_lines(&self) -> Vec<PatchLine> {
        let mut lines = vec![PatchLine {
            kind: PatchLineKind::HunkHeader,
            content: self.format_header_line(),
        }];
        lines.extend(self.body_lines.clone());
        lines
    }

    /// Returns the header line, including the unified diff header and the context
    pub fn format_header_line(&self) -> String {
        format!("{}{}", self.format_header_start(), self.header_context)
    }

    /// Returns the first part of the header line i.e. the unified diff part
    pub fn format_header_start(&self) -> String {
        let new_length_display = if self.new_length() != 1 {
            format!(",{}", self.new_length())
        } else {
            String::new()
        };

        format!(
            "@@ -{},{} +{}{} @@",
            self.old_start,
            self.old_length(),
            self.new_start,
            new_length_display
        )
    }
}

/// A parsed patch
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patch {
    /// header of the patch (e.g. diff --git a/filename b/filename, index ..., --- a/filename, +++ b/filename)
    pub header: Vec<String>,
    /// hunks of the patch
    pub hunks: Vec<Hunk>,
}

impl Patch {
    /// Parse a patch string into a Patch struct
    pub fn parse(patch_str: &str) -> Patch {
        let lines: Vec<&str> = patch_str.trim_end_matches('\n').lines().collect();

        let mut hunks = Vec::new();
        let mut patch_header = Vec::new();
        let mut current_hunk: Option<Hunk> = None;

        for line in lines {
            if line.starts_with("@@") {
                if let Some(hunk) = current_hunk.take() {
                    hunks.push(hunk);
                }

                let (old_start, new_start, header_context) = header_info(line);

                current_hunk = Some(Hunk {
                    old_start,
                    new_start,
                    header_context,
                    body_lines: Vec::new(),
                });
            } else if let Some(ref mut hunk) = current_hunk {
                hunk.body_lines.push(new_hunk_line(line));
            } else {
                patch_header.push(line.to_string());
            }
        }

        if let Some(hunk) = current_hunk {
            hunks.push(hunk);
        }

        Patch {
            hunks,
            header: patch_header,
        }
    }

    /// Returns the lines of the patch
    pub fn lines(&self) -> Vec<PatchLine> {
        let mut lines = Vec::new();

        for line in &self.header {
            lines.push(PatchLine {
                kind: PatchLineKind::PatchHeader,
                content: line.clone(),
            });
        }

        for hunk in &self.hunks {
            lines.extend(hunk.all_lines());
        }

        lines
    }

    /// Returns true if the patch contains any changes
    pub fn contains_changes(&self) -> bool {
        self.hunks.iter().any(|hunk| hunk.contains_changes())
    }

    /// Returns the length of the patch in lines
    pub fn line_count(&self) -> usize {
        let mut count = self.header.len();
        for hunk in &self.hunks {
            count += hunk.line_count();
        }
        count
    }

    /// Returns the number of hunks
    pub fn hunk_count(&self) -> usize {
        self.hunks.len()
    }

    /// Returns the old-file starting line number of the hunk containing the given
    /// patch line index. Returns 0 if the line is not inside any hunk.
    pub fn hunk_old_start_for_line(&self, idx: usize) -> usize {
        match self.hunk_containing_line(idx) {
            -1 => 0,
            hunk_idx => self.hunks[hunk_idx as usize].old_start,
        }
    }

    /// Returns the patch line index of the first line in the given hunk
    pub fn hunk_start_idx(&self, hunk_index: usize) -> usize {
        let hunk_index = hunk_index.clamp(0, self.hunks.len().saturating_sub(1));

        let mut result = self.header.len();
        for i in 0..hunk_index {
            result += self.hunks[i].line_count();
        }
        result
    }

    /// Returns the patch line index of the last line in the given hunk
    pub fn hunk_end_idx(&self, hunk_index: usize) -> usize {
        let hunk_index = hunk_index.clamp(0, self.hunks.len().saturating_sub(1));
        self.hunk_start_idx(hunk_index) + self.hunks[hunk_index].line_count() - 1
    }

    /// Returns hunk index containing the line at the given patch line index
    pub fn hunk_containing_line(&self, idx: usize) -> i32 {
        for (hunk_idx, hunk) in self.hunks.iter().enumerate() {
            let hunk_start_idx = self.hunk_start_idx(hunk_idx);
            if idx >= hunk_start_idx && idx < hunk_start_idx + hunk.line_count() {
                return hunk_idx as i32;
            }
        }
        -1
    }

    /// Takes a line index in the patch and returns the line number in the new file.
    /// If the line is a header line, returns 1.
    /// If the line is a hunk header line, returns the first file line number in that hunk.
    /// If the line is out of range below, returns the last file line number in the last hunk.
    pub fn line_number_of_line(&self, idx: usize) -> usize {
        if idx < self.header.len() || self.hunks.is_empty() {
            return 1;
        }

        let hunk_idx = self.hunk_containing_line(idx);
        // cursor out of range, return last file line number
        if hunk_idx == -1 {
            let last_hunk = self.hunks.last().unwrap();
            return last_hunk.new_start + last_hunk.new_length() - 1;
        }

        let hunk_idx = hunk_idx as usize;
        let hunk = &self.hunks[hunk_idx];
        let hunk_start_idx = self.hunk_start_idx(hunk_idx);
        let idx_in_hunk = idx - hunk_start_idx;

        if idx_in_hunk == 0 {
            return hunk.new_start;
        }

        let lines = &hunk.body_lines[..idx_in_hunk.saturating_sub(1)];
        let offset = n_lines_with_kind(lines, &[PatchLineKind::Addition, PatchLineKind::Context]);
        hunk.new_start + offset
    }

    /// Returns the patch line index of the next change (addition or deletion)
    pub fn get_next_change_idx(&self, idx: usize) -> usize {
        let idx = idx.clamp(0, self.line_count().saturating_sub(1));
        let lines = self.lines();

        for i in 0..lines.len().saturating_sub(idx) {
            let line = &lines[i + idx];
            if line.is_change() {
                return i + idx;
            }
        }

        // there are no changes from the cursor onwards so we'll return the last change
        for i in (0..lines.len()).rev() {
            if lines[i].is_change() {
                return i;
            }
        }

        0
    }

    /// Adjust the given line number (one-based) according to the current patch.
    pub fn adjust_line_number(&self, line_number: usize) -> usize {
        let mut adjusted_line_number = line_number;
        for hunk in &self.hunks {
            if hunk.old_start >= line_number {
                break;
            }

            if hunk.old_start + hunk.old_length() > line_number {
                return hunk.new_start;
            }

            adjusted_line_number += hunk.new_length().saturating_sub(hunk.old_length());
        }

        adjusted_line_number
    }

    /// Returns true if patch is a single hunk for the whole file
    pub fn is_single_hunk_for_whole_file(&self) -> bool {
        if self.hunks.len() != 1 {
            return false;
        }

        let body_lines = &self.hunks[0].body_lines;
        n_lines_with_kind(body_lines, &[PatchLineKind::Deletion, PatchLineKind::Context]) == 0
            || n_lines_with_kind(body_lines, &[PatchLineKind::Addition, PatchLineKind::Context]) == 0
    }
}

/// Parse hunk header info: @@ -start,lines +start,lines @@ context
fn header_info(header: &str) -> (usize, usize, String) {
    let re = Regex::new(r"^@@ -(\d+)[^\+]+\+(\d+)[^@]+@@(.*)$").unwrap();
    if let Some(captures) = re.captures(header) {
        let old_start: usize = captures.get(1).unwrap().as_str().parse().unwrap();
        let new_start: usize = captures.get(2).unwrap().as_str().parse().unwrap();
        let header_context = captures.get(3).unwrap().as_str().to_string();
        (old_start, new_start, header_context)
    } else {
        (0, 0, String::new())
    }
}

/// Create a new hunk line from a patch line string
fn new_hunk_line(line: &str) -> PatchLine {
    if line.is_empty() {
        return PatchLine {
            kind: PatchLineKind::Context,
            content: String::new(),
        };
    }

    let first_char = line.chars().next().unwrap_or(' ');
    let kind = parse_first_char(first_char);

    PatchLine {
        kind,
        content: line.to_string(),
    }
}

/// Parse the first character to determine line kind
fn parse_first_char(first_char: char) -> PatchLineKind {
    match first_char {
        ' ' => PatchLineKind::Context,
        '+' => PatchLineKind::Addition,
        '-' => PatchLineKind::Deletion,
        '\\' => PatchLineKind::NewlineMessage,
        _ => PatchLineKind::Context,
    }
}

/// Transform options corresponding to Go's TransformOpts
pub struct TransformOpts {
    /// Create a patch that will be applied in reverse with `git apply --reverse`
    pub reverse: bool,
    /// Replace the original header with one referring to this file name
    pub file_name_override: Option<String>,
    /// Treat new files as diffs against an empty file
    pub turn_added_files_into_diff_against_empty_file: bool,
    /// Indices of lines that should be included in the patch
    pub included_line_indices: Vec<usize>,
}

/// Transform a patch according to the given options
pub fn transform(patch: &Patch, opts: TransformOpts) -> Patch {
    let transformer = PatchTransformer {
        patch,
        opts,
    };
    transformer.transform()
}

struct PatchTransformer<'a> {
    patch: &'a Patch,
    opts: TransformOpts,
}

impl<'a> PatchTransformer<'a> {
    fn transform(&self) -> Patch {
        let header = self.transform_header();
        let hunks = self.transform_hunks();

        Patch { header, hunks }
    }

    fn transform_header(&self) -> Vec<String> {
        if let Some(ref file_name) = self.opts.file_name_override {
            return vec![
                format!("--- a/{}", file_name),
                format!("+++ b/{}", file_name),
            ];
        }

        if self.opts.turn_added_files_into_diff_against_empty_file {
            let mut result = Vec::with_capacity(self.patch.header.len());
            for (idx, line) in self.patch.header.iter().enumerate() {
                if line.starts_with("new file mode") {
                    continue;
                }
                if *line == "--- /dev/null"
                    && self
                        .patch
                        .header
                        .get(idx + 1)
                        .map(|l| l.starts_with("+++ b/"))
                        .unwrap_or(false)
                {
                    let next_line = self.patch.header.get(idx + 1).unwrap();
                    result.push(format!("--- a/{}", &next_line[6..]));
                } else {
                    result.push(line.clone());
                }
            }
            return result;
        }

        self.patch.header.clone()
    }

    fn transform_hunks(&self) -> Vec<Hunk> {
        let mut new_hunks = Vec::new();
        let mut start_offset = 0;

        for (i, hunk) in self.patch.hunks.iter().enumerate() {
            let (new_start_offset, formatted_hunk) =
                self.transform_hunk(hunk, start_offset, self.patch.hunk_start_idx(i));
            if formatted_hunk.contains_changes() {
                new_hunks.push(formatted_hunk);
            }
            start_offset = new_start_offset;
        }

        new_hunks
    }

    fn transform_hunk(&self, hunk: &Hunk, start_offset: i64, first_line_idx: usize) -> (i64, Hunk) {
        let new_lines = self.transform_hunk_lines(hunk, first_line_idx);
        let (new_new_start, new_start_offset) =
            self.transform_hunk_header(&new_lines, hunk.old_start, start_offset);

        let new_hunk = Hunk {
            body_lines: new_lines,
            old_start: hunk.old_start,
            new_start: new_new_start,
            header_context: hunk.header_context.clone(),
        };

        (new_start_offset, new_hunk)
    }

    fn transform_hunk_lines(&self, hunk: &Hunk, first_line_idx: usize) -> Vec<PatchLine> {
        let mut skipped_newline_message_index: Option<usize> = None;
        let mut new_lines = Vec::new();
        let mut pending_context: Vec<PatchLine> = Vec::new();
        let mut did_see_unselected_new_file_line = false;

        for (i, line) in hunk.body_lines.iter().enumerate() {
            let line_idx = i + first_line_idx + 1; // plus one for header line
            if line.content.is_empty() {
                break;
            }
            let is_line_selected = self.opts.included_line_indices.contains(&line_idx);

            if line.kind == PatchLineKind::Context {
                flush_pending_context(&mut new_lines, &mut pending_context);
                did_see_unselected_new_file_line = false;
                new_lines.push(line.clone());
                continue;
            }

            if line.kind == PatchLineKind::NewlineMessage {
                if skipped_newline_message_index != Some(line_idx) {
                    flush_pending_context(&mut new_lines, &mut pending_context);
                    new_lines.push(line.clone());
                }
                continue;
            }

            let is_old_file_line = (line.kind == PatchLineKind::Deletion && !self.opts.reverse)
                || (line.kind == PatchLineKind::Addition && self.opts.reverse);

            if is_line_selected {
                // Selected "old-file" lines must flush pending context first
                if is_old_file_line || did_see_unselected_new_file_line {
                    flush_pending_context(&mut new_lines, &mut pending_context);
                }
                new_lines.push(line.clone());
                continue;
            }

            if is_old_file_line {
                let content = if line.content.len() > 1 {
                    format!(" {}", &line.content[1..])
                } else {
                    String::new()
                };
                pending_context.push(PatchLine {
                    kind: PatchLineKind::Context,
                    content,
                });
                continue;
            }

            did_see_unselected_new_file_line = true;

            if line.kind == PatchLineKind::Addition {
                // we don't want to include the 'newline at end of file' line if it involves an addition we're not including
                skipped_newline_message_index = Some(line_idx + 1);
            }
        }

        flush_pending_context(&mut new_lines, &mut pending_context);

        new_lines
    }

    fn transform_hunk_header(
        &self,
        new_body_lines: &[PatchLine],
        old_start: usize,
        start_offset: i64,
    ) -> (usize, i64) {
        let old_length =
            n_lines_with_kind(new_body_lines, &[PatchLineKind::Context, PatchLineKind::Deletion]);
        let new_length =
            n_lines_with_kind(new_body_lines, &[PatchLineKind::Context, PatchLineKind::Addition]);

        let new_start_offset = if old_length == 0 {
            1
        } else if new_length == 0 {
            -1
        } else {
            0
        };

        let new_start = old_start as i64 + start_offset + new_start_offset;

        let final_start_offset = start_offset + new_length as i64 - old_length as i64;

        (new_start as usize, final_start_offset)
    }
}

fn flush_pending_context(new_lines: &mut Vec<PatchLine>, pending_context: &mut Vec<PatchLine>) {
    new_lines.extend(pending_context.drain(..));
}

/// Format a patch as plain text
pub fn format_plain(patch: &Patch) -> String {
    if !patch.contains_changes() {
        return String::new();
    }

    let mut result = String::new();

    for line in &patch.header {
        result.push_str(line);
        result.push('\n');
    }

    for hunk in &patch.hunks {
        result.push_str(&hunk.format_header_start());
        result.push('\n');
        result.push_str(&hunk.header_context);
        result.push('\n');

        for line in &hunk.body_lines {
            result.push_str(&line.content);
            result.push('\n');
        }
    }

    result
}

/// Format a range of lines from a patch as plain text (inclusive)
pub fn format_range_plain(patch: &Patch, start_idx: usize, end_idx: usize) -> String {
    let lines = patch.lines();
    let range_lines = &lines[start_idx..=end_idx.min(lines.len().saturating_sub(1))];
    range_lines
        .iter()
        .map(|line| format!("{}\n", line.content))
        .collect()
}

/// Format view options
pub struct FormatViewOpts {
    /// Line indices for tagged lines (e.g. lines added to a custom patch)
    pub inc_line_indices: Vec<usize>,
}

/// Get patch line style (for formatting)
fn patch_line_style(line: &PatchLine) -> &'static str {
    match line.kind {
        PatchLineKind::Addition => "green",
        PatchLineKind::Deletion => "red",
        _ => "default",
    }
}

/// Expand a range from start to end (inclusive) into a vector of indices
pub fn expand_range(start: usize, end: usize) -> Vec<usize> {
    (start..=end).collect()
}

/// Patch status corresponding to Go's PatchStatus
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchStatus {
    /// Unselected means the commit file has not been added to the patch
    Unselected,
    /// Whole means the whole diff of a file is included
    Whole,
    /// Part means only specific lines are included
    Part,
}

/// File info for patch building
#[derive(Debug, Clone)]
struct FileInfo {
    mode: PatchStatus,
    included_line_indices: Vec<usize>,
    diff: String,
}

/// PatchBuilder manages building patches for commits
/// Corresponds to Go's PatchBuilder
pub struct PatchBuilder {
    /// The commit hash (or stash ref)
    to: String,
    from: String,
    reverse: bool,
    /// Whether we can modify commits
    can_rebase: bool,
    /// Map of filename to file info
    file_info_map: HashMap<String, FileInfo>,
}

use std::collections::HashMap;

impl PatchBuilder {
    /// Create a new PatchBuilder
    pub fn new() -> Self {
        Self {
            to: String::new(),
            from: String::new(),
            reverse: false,
            can_rebase: false,
            file_info_map: HashMap::new(),
        }
    }

    /// Start building a patch
    pub fn start(&mut self, from: String, to: String, reverse: bool, can_rebase: bool) {
        self.to = to;
        self.from = from;
        self.reverse = reverse;
        self.can_rebase = can_rebase;
        self.file_info_map.clear();
    }

    /// Add a file's whole diff to the patch
    pub fn add_file_whole(&mut self, filename: &str) {
        if let Some(info) = self.file_info_map.get_mut(filename) {
            if info.mode != PatchStatus::Whole {
                info.mode = PatchStatus::Whole;
                let line_count = info.diff.lines().count();
                info.included_line_indices = (0..line_count).collect();
            }
        }
    }

    /// Remove a file from the patch
    pub fn remove_file(&mut self, filename: &str) {
        if let Some(info) = self.file_info_map.get_mut(filename) {
            info.mode = PatchStatus::Unselected;
            info.included_line_indices.clear();
        }
    }

    /// Add a line range to the patch
    pub fn add_file_line_range(&mut self, filename: &str, line_indices: Vec<usize>) {
        if let Some(info) = self.file_info_map.get_mut(filename) {
            info.mode = PatchStatus::Part;
            for idx in line_indices {
                if !info.included_line_indices.contains(&idx) {
                    info.included_line_indices.push(idx);
                }
            }
        }
    }

    /// Remove a line range from the patch
    pub fn remove_file_line_range(&mut self, filename: &str, line_indices: Vec<usize>) {
        if let Some(info) = self.file_info_map.get_mut(filename) {
            info.mode = PatchStatus::Part;
            info.included_line_indices
                .retain(|idx| !line_indices.contains(idx));
            if info.included_line_indices.is_empty() {
                self.remove_file(filename);
            }
        }
    }

    /// Get file status
    pub fn get_file_status(&self, filename: &str, parent: &str) -> PatchStatus {
        if parent != self.to {
            return PatchStatus::Unselected;
        }

        self.file_info_map
            .get(filename)
            .map(|info| info.mode)
            .unwrap_or(PatchStatus::Unselected)
    }

    /// Get included line indices for a file
    pub fn get_file_inc_line_indices(&self, filename: &str) -> Option<Vec<usize>> {
        self.file_info_map
            .get(filename)
            .map(|info| info.included_line_indices.clone())
    }

    /// Check if patch is empty
    pub fn is_empty(&self) -> bool {
        for info in self.file_info_map.values() {
            if info.mode == PatchStatus::Whole
                || (info.mode == PatchStatus::Part && !info.included_line_indices.is_empty())
            {
                return false;
            }
        }
        true
    }

    /// Check if a new patch is required
    pub fn new_patch_required(&self, from: &str, to: &str, reverse: bool) -> bool {
        from != self.from || to != self.to || reverse != self.reverse
    }

    /// Get all files in the patch
    pub fn all_files_in_patch(&self) -> Vec<&String> {
        self.file_info_map.keys().collect()
    }

    /// Clear the patch
    pub fn reset(&mut self) {
        self.to.clear();
        self.file_info_map.clear();
    }

    /// Check if patch builder is active
    pub fn active(&self) -> bool {
        !self.to.is_empty()
    }

    /// Internal: set file diff for a filename
    pub(crate) fn set_file_diff(&mut self, filename: String, diff: String) {
        self.file_info_map.insert(
            filename,
            FileInfo {
                mode: PatchStatus::Unselected,
                included_line_indices: Vec::new(),
                diff,
            },
        );
    }
}

impl Default for PatchBuilder {
    fn default() -> Self {
        Self::new()
    }
}
