use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

static DECOLORISE_CACHE: LazyLock<
    RwLock<HashMap<String, String>>,
    fn() -> RwLock<HashMap<String, String>>,
> = LazyLock::new(|| RwLock::new(HashMap::new()));

pub fn decolorise(input: &str) -> String {
    // Check cache first
    {
        let cache = DECOLORISE_CACHE.read().unwrap();
        if let Some(cached) = cache.get(input) {
            return cached.clone();
        }
    }

    let re = regex::Regex::new(r"\x1B\[([0-9]{1,3}(;[0-9]{1,3})*)?[mGK]").unwrap();
    let link_re = regex::Regex::new(r"\x1B]8;[^;]*;(.*?)(\x1B.|\x07)").unwrap();

    let mut result = input.to_string();
    result = re.replace_all(&result, "").to_string();
    result = link_re.replace_all(&result, "$1").to_string();

    // Store in cache
    {
        let mut cache = DECOLORISE_CACHE.write().unwrap();
        cache.insert(input.to_string(), result.clone());
    }

    result
}

/// Returns true if the given string is a valid hex color value.
/// Valid formats: #RGB or #RRGGBB (e.g., #fff or #ffffff)
pub fn is_valid_hex_value(value: &str) -> bool {
    if value.len() != 4 && value.len() != 7 {
        return false;
    }

    if !value.starts_with('#') {
        return false;
    }

    value[1..]
        .chars()
        .all(|c| matches!(c, '0'..='9' | 'a'..='f' | 'A'..='F'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_hex_value_valid_short() {
        assert!(is_valid_hex_value("#fff"));
        assert!(is_valid_hex_value("#000"));
        assert!(is_valid_hex_value("#ABC"));
        assert!(is_valid_hex_value("#FFF"));
    }

    #[test]
    fn test_is_valid_hex_value_valid_long() {
        assert!(is_valid_hex_value("#ffffff"));
        assert!(is_valid_hex_value("#000000"));
        assert!(is_valid_hex_value("#aabbcc"));
        assert!(is_valid_hex_value("#AABBCC"));
        assert!(is_valid_hex_value("#123456"));
    }

    #[test]
    fn test_is_valid_hex_value_invalid() {
        assert!(!is_valid_hex_value("#ff")); // too short
        assert!(!is_valid_hex_value("#fffff")); // wrong length
        assert!(!is_valid_hex_value("#gggggg")); // invalid chars
        assert!(!is_valid_hex_value("ffffff")); // missing #
        assert!(!is_valid_hex_value("")); // empty
        assert!(!is_valid_hex_value("#")); // just hash
    }

    #[test]
    fn test_decolorise_no_colors() {
        assert_eq!(decolorise("hello world"), "hello world");
    }

    #[test]
    fn test_decolorise_with_colors() {
        // Basic color codes
        assert_eq!(decolorise("\x1B[31mred\x1B[0m"), "red");
        assert_eq!(decolorise("\x1B[1;32mgreen\x1B[0m"), "green");
    }

    #[test]
    fn test_decolorise_caching() {
        let input = "test string";
        let first = decolorise(input);
        let second = decolorise(input);
        assert_eq!(first, second);
    }

    #[test]
    fn test_decolorise_bold() {
        assert_eq!(decolorise("\x1B[1mbold\x1B[0m"), "bold");
    }

    #[test]
    fn test_decolorise_link() {
        // Link escape sequence: \x1B]8;..;url\x1B. or \x1B]8;..;url\x07
        assert_eq!(
            decolorise("\x1B]8;;https://example.com\x1B.Example\x1B]8;;\x1B."),
            "Example"
        );
        assert_eq!(
            decolorise("\x1B]8;;https://example.com\x07Link\x1B]8;;\x07"),
            "Link"
        );
    }
}
