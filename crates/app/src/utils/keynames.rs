//! Key name constants and translations.
//!
//! NOTE: if you make changes to this table, be sure to update
//! docs/keybindings/Custom_Keybindings.md as well

use std::collections::HashMap;

/// Maps key labels to their display names.
pub fn label_by_key() -> HashMap<&'static str, &'static str> {
    let mut map = HashMap::new();
    map.insert("f1", "<f1>");
    map.insert("f2", "<f2>");
    map.insert("f3", "<f3>");
    map.insert("f4", "<f4>");
    map.insert("f5", "<f5>");
    map.insert("f6", "<f6>");
    map.insert("f7", "<f7>");
    map.insert("f8", "<f8>");
    map.insert("f9", "<f9>");
    map.insert("f10", "<f10>");
    map.insert("f11", "<f11>");
    map.insert("f12", "<f12>");
    map.insert("insert", "<insert>");
    map.insert("delete", "<delete>");
    map.insert("home", "<home>");
    map.insert("end", "<end>");
    map.insert("pageup", "<pgup>");
    map.insert("pagedown", "<pgdown>");
    map.insert("up", "<up>");
    map.insert("shift+up", "<s-up>");
    map.insert("down", "<down>");
    map.insert("shift+down", "<s-down>");
    map.insert("left", "<left>");
    map.insert("right", "<right>");
    map.insert("tab", "<tab>");
    map.insert("shift+tab", "<backtab>");
    map.insert("enter", "<enter>");
    map.insert("alt+enter", "<a-enter>");
    map.insert("esc", "<esc>");
    map.insert("backspace", "<backspace>");
    map.insert("ctrl+@", "<c-space>");
    map.insert("ctrl+/", "<c-/>");
    map.insert("space", "<space>");
    map.insert("ctrl+a", "<c-a>");
    map.insert("ctrl+b", "<c-b>");
    map.insert("ctrl+c", "<c-c>");
    map.insert("ctrl+d", "<c-d>");
    map.insert("ctrl+e", "<c-e>");
    map.insert("ctrl+f", "<c-f>");
    map.insert("ctrl+g", "<c-g>");
    map.insert("ctrl+j", "<c-j>");
    map.insert("ctrl+k", "<c-k>");
    map.insert("ctrl+l", "<c-l>");
    map.insert("ctrl+m", "<c-m>");
    map.insert("ctrl+n", "<c-n>");
    map.insert("ctrl+o", "<c-o>");
    map.insert("ctrl+p", "<c-p>");
    map.insert("ctrl+q", "<c-q>");
    map.insert("ctrl+r", "<c-r>");
    map.insert("ctrl+s", "<c-s>");
    map.insert("ctrl+t", "<c-t>");
    map.insert("ctrl+u", "<c-u>");
    map.insert("ctrl+v", "<c-v>");
    map.insert("ctrl+w", "<c-w>");
    map.insert("ctrl+x", "<c-x>");
    map.insert("ctrl+y", "<c-y>");
    map.insert("ctrl+z", "<c-z>");
    map.insert("ctrl+\\", "<c-4>");
    map.insert("ctrl+]", "<c-5>");
    map.insert("ctrl+6", "<c-6>");
    map.insert("ctrl+8", "<c-8>");
    map
}

/// Maps display names back to key labels.
pub fn key_by_label() -> HashMap<&'static str, &'static str> {
    label_by_key()
        .into_iter()
        .map(|(k, v)| (v, k))
        .collect()
}

/// Validates if a keybinding key is valid.
pub fn is_valid_keybinding_key(key: &str) -> bool {
    if key == "<disabled>" {
        return true;
    }

    let rune_count = key.chars().count();
    if rune_count > 1 {
        let key_by_label = key_by_label();
        key_by_label.contains_key(&key.to_lowercase().as_str())
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_keybinding_key_disabled() {
        assert!(is_valid_keybinding_key("<disabled>"));
    }

    #[test]
    fn test_is_valid_keybinding_key_single_char() {
        assert!(is_valid_keybinding_key("a"));
        assert!(is_valid_keybinding_key("1"));
    }

    #[test]
    fn test_is_valid_keybinding_key_known_key() {
        assert!(is_valid_keybinding_key("<enter>"));
        assert!(is_valid_keybinding_key("<esc>"));
        assert!(is_valid_keybinding_key("<up>"));
    }

    #[test]
    fn test_label_by_key_contains_common_keys() {
        let map = label_by_key();
        assert_eq!(map.get("f1"), Some(&"<f1>"));
        assert_eq!(map.get("enter"), Some(&"<enter>"));
        assert_eq!(map.get("esc"), Some(&"<esc>"));
    }
}
