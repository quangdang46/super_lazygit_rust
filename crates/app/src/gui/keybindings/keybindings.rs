// Ported from ./references/lazygit-master/pkg/gui/keybindings/keybindings.go

use crate::gui::keybindings::keynames::{key_by_label, key_to_char};

/// Get the label for a key by its name (e.g., "q" -> "q", "<esc>" -> "<esc>")
pub fn label(name: &str) -> String {
    label_from_key(&get_key(name))
}

/// Get the label from a key string
pub fn label_from_key(key: &str) -> String {
    if key.is_empty() {
        return String::new();
    }

    // Try to parse as a label directly
    let key_lower = key.to_lowercase();
    if let Some(&code) = key_by_label().get(key_lower.as_str()) {
        return key_to_char(code).to_string();
    }

    // Single character keys are returned as-is
    if key.len() == 1 {
        return key.to_string();
    }

    String::new()
}

/// Get the key code for a label (e.g., "<esc>" -> 0x1B)
pub fn get_key(key: &str) -> String {
    if key == "<disabled>" {
        return String::new();
    }

    let rune_count = key.len();
    if rune_count > 1 {
        let key_lower = key.to_lowercase();
        if let Some(code) = key_by_label().get(key_lower.as_str()) {
            return code.to_string();
        }
    } else if rune_count == 1 {
        return key.to_string();
    }

    String::new()
}
