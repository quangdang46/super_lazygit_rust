// Ported from ./references/lazygit-master/pkg/gui/context/search_trait.go

use crate::types::context::ISearchableContext;
use crate::context::history_trait::SearchHistory;

pub struct SearchTrait {
    pub c: ContextCommon,
    pub search_history: SearchHistory,
    search_string: String,
}

pub struct ContextCommon;

impl SearchTrait {
    pub fn new(c: ContextCommon) -> Self {
        Self {
            c,
            search_history: SearchHistory::new(),
            search_string: String::new(),
        }
    }

    pub fn get_search_string(&self) -> String {
        self.search_string.clone()
    }

    pub fn set_search_string(&mut self, search_string: String) {
        self.search_string = search_string;
    }

    pub fn clear_search_string(&mut self) {
        self.set_search_string(String::new());
    }

    // used for type switch
    pub fn is_searchable_context(&self) {}

    pub fn render_search_status(&self, index: usize, total: usize) -> String {
        // Placeholder - would use theming and i18n in full implementation
        if total == 0 {
            format!("No matches for '{}'", self.search_string)
        } else {
            format!(
                "Match {}/{} for '{}'",
                index + 1,
                total,
                self.search_string
            )
        }
    }

    pub fn is_searching(&self) -> bool {
        !self.search_string.is_empty()
    }
}
