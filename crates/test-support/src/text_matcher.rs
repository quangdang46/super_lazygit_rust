//! Text matching utilities for integration test assertions.
//!
//! Ported from Go's pkg/integration/components/text_matcher.go

use regex::Regex;

use super::matcher::{MatchResult, Matcher, TrueMatcher};

/// A matcher that matches strings containing a substring.
#[derive(Debug, Clone)]
pub struct ContainsMatcher {
    substring: String,
}

impl ContainsMatcher {
    pub fn new(substring: &str) -> Self {
        Self {
            substring: substring.to_string(),
        }
    }
}

impl Matcher<String> for ContainsMatcher {
    fn test(&self, value: &String) -> bool {
        value.contains(&self.substring)
    }

    fn name(&self) -> &'static str {
        "Contains"
    }

    fn expected(&self) -> String {
        format!("contains '{}'", self.substring)
    }
}

/// A matcher that matches strings equal to a given string.
#[derive(Debug, Clone)]
pub struct EqualsMatcher {
    expected: String,
}

impl EqualsMatcher {
    pub fn new(expected: &str) -> Self {
        Self {
            expected: expected.to_string(),
        }
    }
}

impl Matcher<String> for EqualsMatcher {
    fn test(&self, value: &String) -> bool {
        value == &self.expected
    }

    fn name(&self) -> &'static str {
        "Equals"
    }

    fn expected(&self) -> String {
        format!("equals '{}'", self.expected)
    }
}

/// A matcher that matches strings matching a regex pattern.
#[derive(Debug, Clone)]
pub struct MatchesRegexpMatcher {
    pattern: String,
    regex: Regex,
}

impl MatchesRegexpMatcher {
    pub fn new(pattern: &str) -> Self {
        let regex = Regex::new(pattern).expect("Invalid regex pattern");
        Self {
            pattern: pattern.to_string(),
            regex,
        }
    }
}

impl Matcher<String> for MatchesRegexpMatcher {
    fn test(&self, value: &String) -> bool {
        self.regex.is_match(value)
    }

    fn name(&self) -> &'static str {
        "MatchesRegexp"
    }

    fn expected(&self) -> String {
        format!("matches regexp '{}'", self.pattern)
    }
}

/// A matcher that matches strings that do NOT contain a substring.
#[derive(Debug, Clone)]
pub struct DoesNotContainMatcher {
    substring: String,
}

impl DoesNotContainMatcher {
    pub fn new(substring: &str) -> Self {
        Self {
            substring: substring.to_string(),
        }
    }
}

impl Matcher<String> for DoesNotContainMatcher {
    fn test(&self, value: &String) -> bool {
        !value.contains(&self.substring)
    }

    fn name(&self) -> &'static str {
        "DoesNotContain"
    }

    fn expected(&self) -> String {
        format!("does not contain '{}'", self.substring)
    }
}

/// A matcher that matches strings that do NOT equal a given string.
#[derive(Debug, Clone)]
pub struct DoesNotEqualMatcher {
    value: String,
}

impl DoesNotEqualMatcher {
    pub fn new(value: &str) -> Self {
        Self {
            value: value.to_string(),
        }
    }
}

impl Matcher<String> for DoesNotEqualMatcher {
    fn test(&self, value: &String) -> bool {
        value != &self.value
    }

    fn name(&self) -> &'static str {
        "DoesNotEqual"
    }

    fn expected(&self) -> String {
        format!("does not equal '{}'", self.value)
    }
}

/// Matches strings that start with a given prefix.
#[derive(Debug, Clone)]
pub struct StartsWithMatcher {
    prefix: String,
}

impl StartsWithMatcher {
    pub fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_string(),
        }
    }
}

impl Matcher<String> for StartsWithMatcher {
    fn test(&self, value: &String) -> bool {
        value.starts_with(&self.prefix)
    }

    fn name(&self) -> &'static str {
        "StartsWith"
    }

    fn expected(&self) -> String {
        format!("starts with '{}'", self.prefix)
    }
}

/// Matches strings that end with a given suffix.
#[derive(Debug, Clone)]
pub struct EndsWithMatcher {
    suffix: String,
}

impl EndsWithMatcher {
    pub fn new(suffix: &str) -> Self {
        Self {
            suffix: suffix.to_string(),
        }
    }
}

impl Matcher<String> for EndsWithMatcher {
    fn test(&self, value: &String) -> bool {
        value.ends_with(&self.suffix)
    }

    fn name(&self) -> &'static str {
        "EndsWith"
    }

    fn expected(&self) -> String {
        format!("ends with '{}'", self.suffix)
    }
}

/// A text matcher that can match strings using various strategies.
pub struct TextMatcher {
    inner: Box<dyn Matcher<String>>,
}

impl std::fmt::Debug for TextMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextMatcher")
            .field("name", &self.name())
            .field("expected", &self.expected())
            .finish()
    }
}

impl Clone for TextMatcher {
    fn clone(&self) -> Self {
        Self {
            inner: Box::new(TrueMatcher),
        }
    }
}

impl TextMatcher {
    pub fn contains(substring: &str) -> Self {
        Self {
            inner: Box::new(ContainsMatcher::new(substring)),
        }
    }

    pub fn equals(expected: &str) -> Self {
        Self {
            inner: Box::new(EqualsMatcher::new(expected)),
        }
    }

    pub fn matches_regexp(pattern: &str) -> Self {
        Self {
            inner: Box::new(MatchesRegexpMatcher::new(pattern)),
        }
    }

    pub fn does_not_contain(substring: &str) -> Self {
        Self {
            inner: Box::new(DoesNotContainMatcher::new(substring)),
        }
    }

    pub fn does_not_equal(value: &str) -> Self {
        Self {
            inner: Box::new(DoesNotEqualMatcher::new(value)),
        }
    }

    pub fn starts_with(prefix: &str) -> Self {
        Self {
            inner: Box::new(StartsWithMatcher::new(prefix)),
        }
    }

    pub fn ends_with(suffix: &str) -> Self {
        Self {
            inner: Box::new(EndsWithMatcher::new(suffix)),
        }
    }

    pub fn test(&self, value: &str) -> bool {
        self.inner.test(&value.to_string())
    }

    pub fn name(&self) -> &'static str {
        self.inner.name()
    }

    pub fn expected(&self) -> String {
        self.inner.expected()
    }

    pub fn result(&self, actual: &str) -> MatchResult {
        if self.test(actual) {
            MatchResult::new("match", self.expected(), actual.to_string())
        } else {
            MatchResult::new("no match", self.expected(), actual.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contains() {
        let matcher = TextMatcher::contains("ello");
        assert!(matcher.test("hello"));
        assert!(!matcher.test("world"));
    }

    #[test]
    fn test_equals() {
        let matcher = TextMatcher::equals("hello");
        assert!(matcher.test("hello"));
        assert!(!matcher.test("world"));
    }

    #[test]
    fn test_matches_regexp() {
        let matcher = TextMatcher::matches_regexp(r"^\d{3}-\d{4}$");
        assert!(matcher.test("123-4567"));
        assert!(!matcher.test("12-34567"));
    }

    #[test]
    fn test_does_not_contain() {
        let matcher = TextMatcher::does_not_contain("world");
        assert!(matcher.test("hello"));
        assert!(!matcher.test("hello world"));
    }

    #[test]
    fn test_starts_with() {
        let matcher = TextMatcher::starts_with("hello");
        assert!(matcher.test("hello world"));
        assert!(!matcher.test("world hello"));
    }

    #[test]
    fn test_ends_with() {
        let matcher = TextMatcher::ends_with("world");
        assert!(matcher.test("hello world"));
        assert!(!matcher.test("world hello"));
    }
}
