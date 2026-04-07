// Ported from ./references/lazygit-master/pkg/gui/context/filtered_list_view_model.go

use crate::context::history_trait::SearchHistory;
use crate::context::list_view_model::ListViewModel;
use std::sync::Mutex;

/// Filtered list view model combining filtered list and list view model
pub struct FilteredListViewModel {
    pub filtered_list: FilteredList,
    pub list_view_model: ListViewModel,
    pub search_history: SearchHistory,
}

impl FilteredListViewModel {
    /// Create a new filtered list view model
    pub fn new() -> Self {
        Self {
            filtered_list: FilteredList::new(),
            list_view_model: ListViewModel::new(),
            search_history: SearchHistory::new(),
        }
    }

    /// Clear the filter
    pub fn clear_filter(&mut self) {
        self.filtered_list.clear_filter();
    }

    /// Get the filter prefix
    pub fn filter_prefix(&self) -> String {
        String::new()
    }

    /// Get length
    pub fn len(&self) -> usize {
        self.list_view_model.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get selected line index
    pub fn get_selected_line_idx(&self) -> usize {
        self.list_view_model.get_selected_line_idx() as usize
    }

    /// Set selection
    pub fn set_selection(&mut self, value: usize) {
        self.list_view_model.set_selection(value as isize);
    }
}

impl Default for FilteredListViewModel {
    fn default() -> Self {
        Self::new()
    }
}

/// Filtered list for filtering items
pub struct FilteredList {
    filtered_indices: Option<Vec<usize>>,
    filter: String,
    mutex: Mutex<()>,
}

impl FilteredList {
    /// Create a new filtered list
    pub fn new() -> Self {
        Self {
            filtered_indices: None,
            filter: String::new(),
            mutex: Mutex::new(()),
        }
    }

    /// Clear filter
    pub fn clear_filter(&mut self) {
        self.filter.clear();
        self.filtered_indices = None;
    }

    /// Check if filtering is active
    pub fn is_filtering(&self) -> bool {
        !self.filter.is_empty()
    }

    /// Get unfiltered length
    pub fn unfiltered_len(&self) -> usize {
        0
    }

    /// Get unfiltered index from filtered index
    pub fn unfiltered_index(&self, filtered_index: usize) -> usize {
        if let Some(ref indices) = self.filtered_indices {
            indices.get(filtered_index).copied().unwrap_or(filtered_index)
        } else {
            filtered_index
        }
    }
}

impl Default for FilteredList {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filtered_list_view_model_new() {
        let model = FilteredListViewModel::new();
        assert!(model.is_empty());
    }

    #[test]
    fn test_clear_filter() {
        let mut model = FilteredListViewModel::new();
        model.clear_filter();
    }

    #[test]
    fn test_filtered_list_new() {
        let list = FilteredList::new();
        assert!(!list.is_filtering());
    }
}
