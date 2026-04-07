// Ported from ./references/lazygit-master/pkg/gui/context/menu_context.go

use crate::types::common::MenuItem;

/// Menu view model for managing menu state
pub struct MenuViewModel {
    menu_items: Vec<MenuItem>,
    prompt: String,
    prompt_lines: Vec<String>,
    column_alignment: Vec<i32>,
    allow_filtering_keybindings: bool,
    keybindings_take_precedence: bool,
}

/// Menu context for displaying menus
pub struct MenuContext {
    pub key: String,
    pub view_model: MenuViewModel,
}

impl MenuContext {
    pub fn new() -> Self {
        Self {
            key: "MENU_CONTEXT_KEY".to_string(),
            view_model: MenuViewModel::new(),
        }
    }
}

impl MenuViewModel {
    pub fn new() -> Self {
        Self {
            menu_items: Vec::new(),
            prompt: String::new(),
            prompt_lines: Vec::new(),
            column_alignment: Vec::new(),
            allow_filtering_keybindings: false,
            keybindings_take_precedence: false,
        }
    }

    /// Set menu items
    pub fn set_menu_items(&mut self, items: Vec<MenuItem>, column_alignment: Vec<i32>) {
        self.menu_items = items;
        self.column_alignment = column_alignment;
    }

    /// Get prompt
    pub fn get_prompt(&self) -> &str {
        &self.prompt
    }

    /// Set prompt
    pub fn set_prompt(&mut self, prompt: String) {
        self.prompt = prompt;
        self.prompt_lines = Vec::new();
    }

    /// Get prompt lines
    pub fn get_prompt_lines(&self) -> &[String] {
        &self.prompt_lines
    }

    /// Set prompt lines
    pub fn set_prompt_lines(&mut self, prompt_lines: Vec<String>) {
        self.prompt_lines = prompt_lines;
    }

    /// Set allow filtering keybindings
    pub fn set_allow_filtering_keybindings(&mut self, allow: bool) {
        self.allow_filtering_keybindings = allow;
    }

    /// Set keybindings take precedence
    pub fn set_keybindings_take_precedence(&mut self, value: bool) {
        self.keybindings_take_precedence = value;
    }

    /// Check if range select is enabled (disabled for menu)
    pub fn range_select_enabled(&self) -> bool {
        false
    }
}

impl Default for MenuContext {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for MenuViewModel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_context_new() {
        let ctx = MenuContext::new();
        assert_eq!(ctx.key, "MENU_CONTEXT_KEY");
    }

    #[test]
    fn test_menu_view_model_new() {
        let view_model = MenuViewModel::new();
        assert!(view_model.menu_items.is_empty());
        assert!(!view_model.allow_filtering_keybindings);
    }

    #[test]
    fn test_menu_view_model_range_select_disabled() {
        let view_model = MenuViewModel::new();
        assert!(!view_model.range_select_enabled());
    }
}
