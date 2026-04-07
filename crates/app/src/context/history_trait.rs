// Ported from ./references/lazygit-master/pkg/gui/context/history_trait.go

use crate::utils::HistoryBuffer;

/// Maintains a list of strings that have previously been searched/filtered for
pub struct SearchHistory {
    history: HistoryBuffer<String>,
}

impl SearchHistory {
    pub fn new() -> Self {
        Self {
            history: HistoryBuffer::new(1000),
        }
    }

    pub fn get_search_history(&self) -> &HistoryBuffer<String> {
        &self.history
    }
}

impl Default for SearchHistory {
    fn default() -> Self {
        Self::new()
    }
}
