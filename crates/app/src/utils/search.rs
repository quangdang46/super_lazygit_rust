use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

#[derive(Debug, Clone)]
pub struct Match {
    pub str: String,
    pub index: usize,
}

pub fn filter_strings(needle: &str, haystack: &[String], use_fuzzy_search: bool) -> Vec<String> {
    if needle.is_empty() {
        return vec![];
    }

    let matches = find(needle, haystack, use_fuzzy_search);
    matches.into_iter().map(|m| m.str).collect()
}

pub fn find(pattern: &str, data: &[String], use_fuzzy_search: bool) -> Vec<Match> {
    if use_fuzzy_search {
        find_fuzzy(pattern, data)
    } else {
        find_substrings(pattern, data)
    }
}

fn find_fuzzy(pattern: &str, data: &[String]) -> Vec<Match> {
    let matcher = SkimMatcherV2::default();
    let mut matches: Vec<(i64, Match)> = data
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            matcher.fuzzy_match(s, pattern).map(|score| {
                (
                    score,
                    Match {
                        str: s.clone(),
                        index: i,
                    },
                )
            })
        })
        .collect();

    matches.sort_by(|a, b| b.0.cmp(&a.0));
    matches.into_iter().map(|(_, m)| m).collect()
}

fn find_substrings(pattern: &str, data: &[String]) -> Vec<Match> {
    let substrings: Vec<&str> = pattern.split_whitespace().collect();

    let mut results = Vec::new();

    for (i, s) in data.iter().enumerate() {
        let mut all_match = true;
        for sub in &substrings {
            if !case_aware_contains(s, sub) {
                all_match = false;
                break;
            }
        }
        if all_match {
            results.push(Match {
                str: s.clone(),
                index: i,
            });
        }
    }

    results
}

pub fn case_aware_contains(haystack: &str, needle: &str) -> bool {
    if contains_uppercase(needle) {
        haystack.contains(needle)
    } else {
        case_insensitive_contains(haystack, needle)
    }
}

pub fn contains_uppercase(s: &str) -> bool {
    s.chars().any(|c| c.is_ascii_uppercase())
}

pub fn case_insensitive_contains(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contains_uppercase() {
        assert!(contains_uppercase("Hello"));
        assert!(contains_uppercase("HELLO"));
        assert!(!contains_uppercase("hello"));
        assert!(!contains_uppercase(""));
    }

    #[test]
    fn test_case_aware_contains_case_sensitive() {
        assert!(case_aware_contains("Hello World", "Hello"));
        assert!(case_aware_contains("Hello World", "hello"));
        assert!(!case_aware_contains("Hello World", "GOODBYE"));
    }

    #[test]
    fn test_case_insensitive_contains() {
        assert!(case_insensitive_contains("Hello World", "hello"));
        assert!(case_insensitive_contains("Hello World", "HELLO"));
        assert!(case_insensitive_contains("Hello World", "world"));
    }

    #[test]
    fn test_find_substrings() {
        let data = vec![
            "apple".to_string(),
            "banana".to_string(),
            "applesauce".to_string(),
            "grape".to_string(),
        ];

        let matches = find_substrings("app", &data);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].str, "apple");
        assert_eq!(matches[1].str, "applesauce");
    }

    #[test]
    fn test_find_substrings_multiple_words() {
        let data = vec![
            "apple pie".to_string(),
            "apple sauce".to_string(),
            "green apple".to_string(),
        ];

        let matches = find_substrings("apple", &data);
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn test_find_fuzzy() {
        let data = vec![
            "apple".to_string(),
            "applesauce".to_string(),
            "banana".to_string(),
        ];

        let matches = find_fuzzy("ap", &data);
        assert!(matches.len() >= 2);
    }

    #[test]
    fn test_filter_strings() {
        let data = vec![
            "apple".to_string(),
            "applesauce".to_string(),
            "banana".to_string(),
        ];

        let result = filter_strings("app", &data, false);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_filter_strings_empty_needle() {
        let data = vec!["apple".to_string(), "banana".to_string()];
        let result = filter_strings("", &data, false);
        assert!(result.is_empty());
    }
}
