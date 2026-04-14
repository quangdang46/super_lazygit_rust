use std::path::Path;

use crate::mergeconflicts::find_conflicts::find_conflicts;
use crate::mergeconflicts::merge_conflict::{available_selections, MergeConflict, Selection};
use crate::utils::io::for_each_line_in_file;
use crate::utils::lines::split_lines;

pub struct State {
    path: String,
    contents: Vec<String>,
    conflicts: Vec<MergeConflict>,
    conflict_index: usize,
    selection_index: usize,
}

impl State {
    pub fn new() -> Self {
        Self {
            conflict_index: 0,
            selection_index: 0,
            conflicts: Vec::new(),
            contents: Vec::new(),
            path: String::new(),
        }
    }

    fn set_conflict_index(&mut self, index: usize) {
        if self.conflicts.is_empty() {
            self.conflict_index = 0;
        } else {
            self.conflict_index = index.clamp(0, self.conflicts.len() - 1);
        }
        self.set_selection_index(self.selection_index);
    }

    fn set_selection_index(&mut self, index: usize) {
        if let Some(selections) = self.available_selections() {
            if !selections.is_empty() {
                self.selection_index = index.clamp(0, selections.len() - 1);
            }
        }
    }

    pub fn select_next_conflict_hunk(&mut self) {
        self.set_selection_index(self.selection_index + 1);
    }

    pub fn select_prev_conflict_hunk(&mut self) {
        self.set_selection_index(self.selection_index.saturating_sub(1));
    }

    pub fn select_next_conflict(&mut self) {
        self.set_conflict_index(self.conflict_index + 1);
    }

    pub fn select_prev_conflict(&mut self) {
        self.set_conflict_index(self.conflict_index.saturating_sub(1));
    }

    fn current_conflict(&self) -> Option<&MergeConflict> {
        if self.conflicts.is_empty() {
            return None;
        }
        self.conflicts.get(self.conflict_index)
    }

    pub fn set_content(&mut self, content: String, path: String) {
        if content == self.get_content() && path == self.path {
            return;
        }

        self.path = path;
        self.contents = vec![];
        self.push_content(content);
    }

    pub fn push_content(&mut self, content: String) {
        self.contents.push(content.clone());
        self.set_conflicts(find_conflicts(&content));
    }

    pub fn get_content(&self) -> String {
        self.contents.last().cloned().unwrap_or_default()
    }

    pub fn get_path(&self) -> &str {
        &self.path
    }

    pub fn undo(&mut self) -> bool {
        if self.contents.len() <= 1 {
            return false;
        }

        self.contents.pop();

        let new_content = self.get_content();
        self.set_conflicts(find_conflicts(&new_content));

        true
    }

    fn set_conflicts(&mut self, conflicts: Vec<MergeConflict>) {
        self.conflicts = conflicts;
        self.set_conflict_index(self.conflict_index);
    }

    pub fn no_conflicts(&self) -> bool {
        self.conflicts.is_empty()
    }

    pub fn selection(&self) -> Selection {
        if let Some(selections) = self.available_selections() {
            if !selections.is_empty() {
                return selections[self.selection_index];
            }
        }
        Selection::Top
    }

    fn available_selections(&self) -> Option<Vec<Selection>> {
        self.current_conflict().map(|c| available_selections(c))
    }

    pub fn all_conflicts_resolved(&self) -> bool {
        self.conflicts.is_empty()
    }

    pub fn reset(&mut self) {
        self.contents = vec![];
        self.path = String::new();
    }

    pub fn reset_conflict_selection(&mut self) {
        self.conflict_index = 0;
    }

    pub fn active(&self) -> bool {
        !self.path.is_empty()
    }

    pub fn get_conflict_middle(&self) -> i32 {
        if let Some(conflict) = self.current_conflict() {
            conflict.target
        } else {
            0
        }
    }

    pub fn content_after_conflict_resolve(&self, selection: Selection) -> (bool, String, std::io::Result<()>) {
        let conflict = match self.current_conflict() {
            Some(c) => c,
            None => return (false, String::new(), Ok(())),
        };

        let mut content = String::new();
        let path = Path::new(&self.path);
        
        if let Err(e) = for_each_line_in_file(path, |line, i| {
            if selection.is_index_to_keep(conflict, i as i32) {
                content.push_str(&line);
            }
        }) {
            return (false, String::new(), Err(e));
        }

        (true, content, Ok(()))
    }

    pub fn get_selected_line(&self) -> i32 {
        let conflict = match self.current_conflict() {
            Some(c) => c,
            None => return 1,
        };
        let selection = self.selection();
        let (start_index, _) = selection.bounds(conflict);
        start_index + 1
    }

    pub fn get_selected_range(&self) -> (i32, i32) {
        let conflict = match self.current_conflict() {
            Some(c) => c,
            None => return (0, 0),
        };
        let selection = self.selection();
        selection.bounds(conflict)
    }

    pub fn plain_render_selected(&self) -> String {
        let (start_index, end_index) = self.get_selected_range();
        let content = self.get_content();
        let content_lines = split_lines(&content);
        
        if end_index + 1 <= content_lines.len() as i32 {
            content_lines[start_index as usize..=end_index as usize].join("\n")
        } else {
            String::new()
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}
