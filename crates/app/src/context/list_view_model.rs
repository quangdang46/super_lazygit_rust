// Ported from ./references/lazygit-master/pkg/gui/context/list_view_model.go

/// List view model for managing list state
pub struct ListViewModel {
    cursor: ListCursor,
}

struct ListCursor {
    selected_idx: isize,
    range_select_mode: RangeSelectMode,
    range_start_idx: isize,
    len: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum RangeSelectMode {
    None,
    Sticky,
    NonSticky,
}

impl ListViewModel {
    pub fn new() -> Self {
        Self {
            cursor: ListCursor::new(),
        }
    }

    /// Get selected line index
    pub fn get_selected_line_idx(&self) -> isize {
        self.cursor.selected_idx
    }

    /// Set selection
    pub fn set_selection(&mut self, value: isize) {
        self.cursor.set_selection(value);
    }

    /// Get selection range
    pub fn get_selection_range(&self) -> (isize, isize) {
        self.cursor.get_selection_range()
    }

    /// Get length
    pub fn len(&self) -> usize {
        self.cursor.len
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl ListCursor {
    fn new() -> Self {
        Self {
            selected_idx: 0,
            range_select_mode: RangeSelectMode::None,
            range_start_idx: 0,
            len: 0,
        }
    }

    fn clamp_value(&self, value: isize) -> isize {
        if self.len > 0 {
            value.clamp(0, self.len as isize - 1)
        } else {
            -1
        }
    }

    fn get_selection_range(&self) -> (isize, isize) {
        if self.range_select_mode != RangeSelectMode::None {
            let start = self.range_start_idx.min(self.selected_idx);
            let end = self.range_start_idx.max(self.selected_idx);
            (start, end)
        } else {
            (self.selected_idx, self.selected_idx)
        }
    }

    fn set_selection(&mut self, value: isize) {
        self.selected_idx = self.clamp_value(value);
        self.range_select_mode = RangeSelectMode::None;
    }
}

impl Default for ListViewModel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_view_model_new() {
        let model = ListViewModel::new();
        assert!(model.is_empty());
    }

    #[test]
    fn test_set_selection() {
        let mut model = ListViewModel::new();
        model.set_selection(0);
        assert_eq!(model.get_selected_line_idx(), 0);
    }
}
