// Ported from ./references/lazygit-master/pkg/gui/patch_exploring/state.go

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SelectMode {
    Line,
    Range,
    Hunk,
}

impl Default for SelectMode {
    fn default() -> Self {
        SelectMode::Line
    }
}

#[derive(Clone, Default)]
pub struct State {
    pub selected_line_idx: i32,
    pub range_start_line_idx: i32,
    pub range_is_sticky: bool,
    pub diff: String,
    pub patch: Option<Patch>,
    pub select_mode: SelectMode,
    pub view_line_indices: Vec<i32>,
    pub patch_line_indices: Vec<i32>,
    pub user_enabled_hunk_mode: bool,
}

pub struct Patch;

impl Patch {
    pub fn parse(_diff: &str) -> Patch {
        Patch
    }

    pub fn contains_changes(&self) -> bool {
        true
    }

    pub fn is_single_hunk_for_whole_file(&self) -> bool {
        false
    }

    pub fn lines(&self) -> Vec<PatchLine> {
        Vec::new()
    }

    pub fn get_next_change_idx(&self, _idx: i32) -> i32 {
        0
    }

    pub fn hunk_old_start_for_line(&self, _idx: i32) -> i32 {
        0
    }

    pub fn hunk_containing_line(&self, _idx: i32) -> i32 {
        0
    }

    pub fn hunk_start_idx(&self, _idx: i32) -> i32 {
        0
    }

    pub fn hunk_end_idx(&self, _idx: i32) -> i32 {
        0
    }

    pub fn line_number_of_line(&self, _idx: i32) -> i32 {
        0
    }

    pub fn format_view(&self, _opts: FormatViewOpts) -> String {
        String::new()
    }

    pub fn format_range_plain(&self, _start: i32, _end: i32) -> String {
        String::new()
    }

    pub fn get_next_change_idx_of_same_included_state(
        &self,
        _idx: i32,
        _included_lines: &[i32],
        _included: bool,
    ) -> (i32, bool) {
        (0, false)
    }
}

pub struct PatchLine;

impl PatchLine {
    pub fn is_addition(&self) -> bool {
        false
    }

    pub fn is_deletion(&self) -> bool {
        false
    }

    pub fn is_change(&self) -> bool {
        false
    }
}

pub struct FormatViewOpts {
    pub inc_line_indices: Vec<i32>,
}

impl State {
    pub fn new_state(
        diff: &str,
        selected_line_idx: i32,
        _view: &View,
        old_state: Option<&State>,
        use_hunk_mode_by_default: bool,
    ) -> Option<Self> {
        if let Some(old) = old_state {
            if diff == old.diff && selected_line_idx == -1 {
                return Some(old.clone());
            }
        }

        let patch = Patch::parse(diff);
        if !patch.contains_changes() {
            return None;
        }

        let view_line_indices = vec![0];
        let patch_line_indices = vec![0];

        let range_start_line_idx = old_state.map(|s| s.range_start_line_idx).unwrap_or(0);

        let mut select_mode = SelectMode::Line;
        if use_hunk_mode_by_default && !patch.is_single_hunk_for_whole_file() {
            select_mode = SelectMode::Hunk;
        }

        let user_enabled_hunk_mode = old_state.map(|s| s.user_enabled_hunk_mode).unwrap_or(false);

        let selected_line_idx = if selected_line_idx >= 0 {
            std::cmp::min(selected_line_idx, (view_line_indices.len() - 1) as i32)
        } else if let Some(old) = old_state {
            let old_patch_line_idx = old.patch_line_indices[old.selected_line_idx as usize];
            let new_patch_line_idx = patch.get_next_change_idx(old_patch_line_idx);
            view_line_indices[new_patch_line_idx as usize]
        } else {
            view_line_indices[patch.get_next_change_idx(0) as usize]
        };

        Some(State {
            patch: Some(patch),
            selected_line_idx,
            select_mode,
            range_start_line_idx,
            range_is_sticky: false,
            diff: diff.to_string(),
            view_line_indices,
            patch_line_indices,
            user_enabled_hunk_mode,
        })
    }

    pub fn on_view_width_changed(&mut self, _view: &View) {
        if !_view.wrap {
            return;
        }

        let selected_patch_line_idx = self.patch_line_indices[self.selected_line_idx as usize];
        let range_start_patch_line_idx = if self.select_mode == SelectMode::Range {
            self.patch_line_indices[self.range_start_line_idx as usize]
        } else {
            selected_patch_line_idx
        };

        self.view_line_indices = vec![0];
        self.patch_line_indices = vec![0];
        self.selected_line_idx = self.view_line_indices[selected_patch_line_idx as usize];
        if self.select_mode == SelectMode::Range {
            self.range_start_line_idx = self.view_line_indices[range_start_patch_line_idx as usize];
        }
    }

    pub fn get_selected_patch_line_idx(&self) -> i32 {
        self.patch_line_indices[self.selected_line_idx as usize]
    }

    pub fn get_selected_view_line_idx(&self) -> i32 {
        self.selected_line_idx
    }

    pub fn get_diff(&self) -> &str {
        &self.diff
    }

    pub fn toggle_select_hunk(&mut self) {
        if self.select_mode == SelectMode::Hunk {
            self.select_mode = SelectMode::Line;
        } else {
            self.select_mode = SelectMode::Hunk;
            self.user_enabled_hunk_mode = true;
            if let Some(ref patch) = self.patch {
                self.selected_line_idx = self.view_line_indices[patch
                    .get_next_change_idx(self.patch_line_indices[self.selected_line_idx as usize])
                    as usize];
            }
        }
    }

    pub fn toggle_sticky_select_range(&mut self) {
        self.toggle_select_range(true);
    }

    pub fn toggle_select_range(&mut self, sticky: bool) {
        if self.selecting_range() {
            self.select_mode = SelectMode::Line;
        } else {
            self.select_mode = SelectMode::Range;
            self.range_start_line_idx = self.selected_line_idx;
            self.range_is_sticky = sticky;
        }
    }

    pub fn set_range_is_sticky(&mut self, value: bool) {
        self.range_is_sticky = value;
    }

    pub fn selecting_hunk(&self) -> bool {
        self.select_mode == SelectMode::Hunk
    }

    pub fn selecting_hunk_enabled_by_user(&self) -> bool {
        self.select_mode == SelectMode::Hunk && self.user_enabled_hunk_mode
    }

    pub fn selecting_range(&self) -> bool {
        self.select_mode == SelectMode::Range
            && (self.range_is_sticky || self.range_start_line_idx != self.selected_line_idx)
    }

    pub fn selecting_line(&self) -> bool {
        self.select_mode == SelectMode::Line
    }

    pub fn set_line_select_mode(&mut self) {
        self.select_mode = SelectMode::Line;
    }

    pub fn dismiss_hunk_select_mode(&mut self) {
        if self.selecting_hunk() {
            self.select_mode = SelectMode::Line;
        }
    }

    pub fn select_line(&mut self, new_selected_line_idx: i32) {
        if self.select_mode == SelectMode::Range && !self.range_is_sticky {
            self.select_mode = SelectMode::Line;
        }
        self.select_line_without_range_check(new_selected_line_idx);
    }

    fn clamp_line_idx(&self, line_idx: i32) -> i32 {
        line_idx.clamp(0, (self.patch_line_indices.len() - 1) as i32)
    }

    fn select_line_without_range_check(&mut self, new_selected_line_idx: i32) {
        self.selected_line_idx = self.clamp_line_idx(new_selected_line_idx);
    }

    pub fn select_new_line_for_range(&mut self, new_selected_line_idx: i32) {
        self.range_start_line_idx = self.clamp_line_idx(new_selected_line_idx);
        self.select_mode = SelectMode::Range;
        self.select_line_without_range_check(new_selected_line_idx);
    }

    pub fn drag_select_line(&mut self, new_selected_line_idx: i32) {
        self.select_mode = SelectMode::Range;
        self.select_line_without_range_check(new_selected_line_idx);
    }

    pub fn cycle_selection(&mut self, forward: bool) {
        if self.selecting_hunk() {
            if forward {
                self.select_next_hunk();
            } else {
                self.select_previous_hunk();
            }
        } else {
            self.cycle_line(forward);
        }
    }

    pub fn select_previous_hunk(&mut self) {
        if let Some(ref patch) = self.patch {
            let patch_lines = patch.lines();
            let patch_line_idx = self.patch_line_indices[self.selected_line_idx as usize];
            let mut next_non_change_line = patch_line_idx;
            while next_non_change_line >= 0
                && patch_lines[next_non_change_line as usize].is_change()
            {
                next_non_change_line -= 1;
            }
            let mut next_change_line = next_non_change_line;
            while next_change_line >= 0 && !patch_lines[next_change_line as usize].is_change() {
                next_change_line -= 1;
            }
            if next_change_line >= 0 {
                self.selected_line_idx = self.view_line_indices[next_change_line as usize];
            }
        }
    }

    pub fn select_next_hunk(&mut self) {
        if let Some(ref patch) = self.patch {
            let patch_lines = patch.lines();
            let patch_line_idx = self.patch_line_indices[self.selected_line_idx as usize];
            let mut next_non_change_line = patch_line_idx;
            while next_non_change_line < patch_lines.len() as i32
                && patch_lines[next_non_change_line as usize].is_change()
            {
                next_non_change_line += 1;
            }
            let mut next_change_line = next_non_change_line;
            while next_change_line < patch_lines.len() as i32
                && !patch_lines[next_change_line as usize].is_change()
            {
                next_change_line += 1;
            }
            if next_change_line < patch_lines.len() as i32 {
                self.selected_line_idx = self.view_line_indices[next_change_line as usize];
            }
        }
    }

    pub fn cycle_line(&mut self, forward: bool) {
        let change = if forward { 1 } else { -1 };
        self.select_line(self.selected_line_idx + change);
    }

    pub fn cycle_range(&mut self, forward: bool) {
        if !self.selecting_range() {
            self.toggle_select_range(false);
        }
        self.set_range_is_sticky(false);
        let change = if forward { 1 } else { -1 };
        self.select_line_without_range_check(self.selected_line_idx + change);
    }

    pub fn current_hunk_bounds(&self) -> (i32, i32) {
        if let Some(ref patch) = self.patch {
            let hunk_idx = patch
                .hunk_containing_line(self.patch_line_indices[self.selected_line_idx as usize]);
            let start = patch.hunk_start_idx(hunk_idx);
            let end = patch.hunk_end_idx(hunk_idx);
            return (start, end);
        }
        (0, 0)
    }

    fn selection_range_for_current_block_of_changes(&self) -> (i32, i32) {
        if let Some(ref patch) = self.patch {
            let patch_lines = patch.lines();
            let patch_line_idx = self.patch_line_indices[self.selected_line_idx as usize];

            let mut patch_start = patch_line_idx;
            while patch_start > 0 && patch_lines[(patch_start - 1) as usize].is_change() {
                patch_start -= 1;
            }

            let mut patch_end = patch_line_idx;
            while patch_end < patch_lines.len() as i32 - 1
                && patch_lines[(patch_end + 1) as usize].is_change()
            {
                patch_end += 1;
            }

            let view_start = self.view_line_indices[patch_start as usize];
            let mut view_end = self.view_line_indices[patch_end as usize];

            while view_end < self.patch_line_indices.len() as i32 - 1
                && self.patch_line_indices[view_end as usize]
                    == self.patch_line_indices[(view_end + 1) as usize]
            {
                view_end += 1;
            }

            return (view_start, view_end);
        }
        (0, 0)
    }

    pub fn selected_view_range(&self) -> (i32, i32) {
        match self.select_mode {
            SelectMode::Hunk => self.selection_range_for_current_block_of_changes(),
            SelectMode::Range => {
                if self.range_start_line_idx > self.selected_line_idx {
                    (self.selected_line_idx, self.range_start_line_idx)
                } else {
                    (self.range_start_line_idx, self.selected_line_idx)
                }
            }
            SelectMode::Line => (self.selected_line_idx, self.selected_line_idx),
        }
    }

    pub fn selected_patch_range(&self) -> (i32, i32) {
        let (start, end) = self.selected_view_range();
        (
            self.patch_line_indices[start as usize],
            self.patch_line_indices[end as usize],
        )
    }

    pub fn line_indices_of_added_or_deleted_lines_in_selected_patch_range(&self) -> Vec<i32> {
        if let Some(ref patch) = self.patch {
            let (view_start, view_end) = self.selected_view_range();
            let patch_start = self.patch_line_indices[view_start as usize];
            let patch_end = self.patch_line_indices[view_end as usize];
            let lines = patch.lines();
            let mut indices = Vec::new();
            for i in patch_start..=patch_end {
                if lines[i as usize].is_change() {
                    indices.push(i);
                }
            }
            return indices;
        }
        Vec::new()
    }

    pub fn current_line_number(&self) -> i32 {
        if let Some(ref patch) = self.patch {
            return patch
                .line_number_of_line(self.patch_line_indices[self.selected_line_idx as usize]);
        }
        0
    }

    pub fn adjust_selected_line_idx(&mut self, change: i32) {
        self.dismiss_hunk_select_mode();
        self.select_line(self.selected_line_idx + change);
    }

    pub fn render_for_line_indices(&self, included_line_indices: Vec<i32>) -> String {
        if let Some(ref patch) = self.patch {
            return patch.format_view(FormatViewOpts {
                inc_line_indices: included_line_indices,
            });
        }
        String::new()
    }

    pub fn plain_render_selected(&self) -> String {
        if let Some(ref patch) = self.patch {
            let (first_line_idx, last_line_idx) = self.selected_patch_range();
            return patch.format_range_plain(first_line_idx, last_line_idx);
        }
        String::new()
    }

    pub fn select_bottom(&mut self) {
        self.dismiss_hunk_select_mode();
        self.select_line((self.patch_line_indices.len() - 1) as i32);
    }

    pub fn select_top(&mut self) {
        self.dismiss_hunk_select_mode();
        self.select_line(0);
    }

    pub fn calculate_origin(&self, current_origin: i32, buffer_height: i32, num_lines: i32) -> i32 {
        let (first_line_idx, last_line_idx) = self.selected_view_range();
        calculate_origin(
            current_origin,
            buffer_height,
            num_lines,
            first_line_idx,
            last_line_idx,
            self.get_selected_view_line_idx(),
            &self.select_mode,
        )
    }

    pub fn select_next_stageable_line_of_same_included_state(
        &mut self,
        included_lines: &[i32],
        included: bool,
    ) {
        if let Some(ref patch) = self.patch {
            let (_, last_line_idx) = self.selected_patch_range();
            let (patch_line_idx, found) = patch.get_next_change_idx_of_same_included_state(
                last_line_idx + 1,
                included_lines,
                included,
            );
            if found {
                self.select_line(self.view_line_indices[patch_line_idx as usize]);
            }
        }
    }
}

pub struct View {
    pub wrap: bool,
    pub editable: bool,
    pub tab_width: i32,
}

pub fn calculate_origin(
    current_origin: i32,
    buffer_height: i32,
    num_lines: i32,
    first_line_idx: i32,
    last_line_idx: i32,
    selected_line_idx: i32,
    _select_mode: &SelectMode,
) -> i32 {
    let mut origin = current_origin;
    if selected_line_idx >= origin + buffer_height {
        origin = selected_line_idx - buffer_height + 1;
    }
    if first_line_idx < origin {
        origin = first_line_idx;
    }
    if last_line_idx < origin {
        origin = last_line_idx;
    }
    if last_line_idx >= origin + buffer_height {
        origin = last_line_idx - buffer_height + 1;
    }
    if first_line_idx >= origin + buffer_height {
        origin = first_line_idx - buffer_height + 1;
    }
    origin = origin.clamp(0, (num_lines - buffer_height).max(0));
    origin
}

pub fn wrap_patch_lines(diff: &str, view: &View) -> (Vec<i32>, Vec<i32>) {
    let trimmed = diff.trim_end_matches('\n');
    let view_line_indices: Vec<i32> = (0..trimmed.lines().count() as i32).collect();
    let patch_line_indices: Vec<i32> = (0..trimmed.lines().count() as i32).collect();
    (view_line_indices, patch_line_indices)
}
