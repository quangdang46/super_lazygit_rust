//! Generic Matcher type for building up assertions.
//!
//! Ported from Go's pkg/integration/components/matcher.go

use std::fmt::Debug;

/// A matcher that can be applied to a value of type T.
pub trait Matcher<T>: Send + Sync {
    /// Returns true if the matcher matches the given value.
    fn test(&self, value: &T) -> bool;

    /// Returns the name of the matcher for error messages.
    fn name(&self) -> &'static str;

    /// Returns the expected value as a string for error messages.
    fn expected(&self) -> String {
        String::new()
    }
}

/// A result of a failed match.
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub message: String,
    pub expected: String,
    pub actual: String,
}

impl MatchResult {
    pub fn new(
        message: impl Into<String>,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        Self {
            message: message.into(),
            expected: expected.into(),
            actual: actual.into(),
        }
    }

    pub fn to_string(&self) -> String {
        format!(
            "FAIL: {}\nExpected: {}\nActual: {}",
            self.message, self.expected, self.actual
        )
    }
}

/// A matcher that always succeeds.
#[derive(Debug, Clone, Copy)]
pub struct TrueMatcher;

impl<T> Matcher<T> for TrueMatcher {
    fn test(&self, _value: &T) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "TrueMatcher"
    }
}

/// A matcher that always fails.
#[derive(Debug, Clone, Copy)]
pub struct FalseMatcher;

impl<T> Matcher<T> for FalseMatcher {
    fn test(&self, _value: &T) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "FalseMatcher"
    }
}

/// Combines multiple matchers with AND logic.
pub struct AllMatcher<T: Debug + Clone + Send + Sync> {
    matchers: Vec<Box<dyn Matcher<T>>>,
}

impl<T: Debug + Clone + Send + Sync> AllMatcher<T> {
    pub fn new(matchers: Vec<Box<dyn Matcher<T>>>) -> Self {
        Self { matchers }
    }
}

impl<T: Debug + Clone + Send + Sync> Matcher<T> for AllMatcher<T> {
    fn test(&self, value: &T) -> bool {
        self.matchers.iter().all(|m| m.test(value))
    }

    fn name(&self) -> &'static str {
        "AllMatcher"
    }
}

/// Combines multiple matchers with OR logic.
pub struct AnyMatcher<T: Debug + Clone + Send + Sync> {
    matchers: Vec<Box<dyn Matcher<T>>>,
}

impl<T: Debug + Clone + Send + Sync> AnyMatcher<T> {
    pub fn new(matchers: Vec<Box<dyn Matcher<T>>>) -> Self {
        Self { matchers }
    }
}

impl<T: Debug + Clone + Send + Sync> Matcher<T> for AnyMatcher<T> {
    fn test(&self, value: &T) -> bool {
        self.matchers.iter().any(|m| m.test(value))
    }

    fn name(&self) -> &'static str {
        "AnyMatcher"
    }
}

/// Negates a matcher.
pub struct NotMatcher<T: Debug + Clone + Send + Sync> {
    inner: Box<dyn Matcher<T>>,
}

impl<T: Debug + Clone + Send + Sync> NotMatcher<T> {
    pub fn new(inner: Box<dyn Matcher<T>>) -> Self {
        Self { inner }
    }
}

impl<T: Debug + Clone + Send + Sync> Matcher<T> for NotMatcher<T> {
    fn test(&self, value: &T) -> bool {
        !self.inner.test(value)
    }

    fn name(&self) -> &'static str {
        "NotMatcher"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_true_matcher() {
        let matcher = TrueMatcher;
        assert!(matcher.test(&42));
        assert!(matcher.test(&"hello"));
    }

    #[test]
    fn test_false_matcher() {
        let matcher = FalseMatcher;
        assert!(!matcher.test(&42));
        assert!(!matcher.test(&"hello"));
    }
}
