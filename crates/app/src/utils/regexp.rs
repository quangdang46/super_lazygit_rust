use regex::Regex;

/// Finds named matches in a string using a compiled regex.
/// Returns a map of capture group names to their matched values.
pub fn find_named_matches(
    regex: &Regex,
    s: &str,
) -> Option<std::collections::HashMap<String, String>> {
    let match_result = regex.find(s)?;

    if match_result.is_empty() {
        return None;
    }

    let captures = regex.captures(s)?;
    let names: Vec<_> = regex.capture_names().collect();

    let mut results = std::collections::HashMap::new();
    let mut capture_idx = 0;
    for name in names {
        if let Some(name_str) = name {
            capture_idx += 1;
            if let Some(value) = captures.get(capture_idx) {
                results.insert(name_str.to_string(), value.as_str().to_string());
            }
        }
    }

    Some(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    #[test]
    fn test_find_named_matches() {
        let regex = Regex::new(r"(?P<area>\w+)/(?P<name>\w+)").unwrap();
        let result = find_named_matches(&regex, "github/test").unwrap();

        assert_eq!(result.get("area").map(|s| s.as_str()), Some("github"));
        assert_eq!(result.get("name").map(|s| s.as_str()), Some("test"));
    }

    #[test]
    fn test_find_named_matches_no_match() {
        let regex = Regex::new(r"(?P<first>\w+)_(?P<second>\w+)").unwrap();
        let result = find_named_matches(&regex, "nomatch");
        assert!(result.is_none());
    }

    #[test]
    fn test_find_named_matches_with_prefix() {
        let regex = Regex::new(r"feature/(?P<branch>.+)").unwrap();
        let result = find_named_matches(&regex, "feature/AB-123").unwrap();

        assert_eq!(result.get("branch").map(|s| s.as_str()), Some("AB-123"));
    }
}
