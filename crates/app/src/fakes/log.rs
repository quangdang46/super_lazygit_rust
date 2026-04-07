/// A fake field logger for testing purposes.
/// Tracks logged errors for assertion in tests.
#[derive(Debug, Default)]
pub struct FakeFieldLogger {
    logged_errors: Vec<String>,
}

impl FakeFieldLogger {
    /// Creates a new FakeFieldLogger.
    pub fn new() -> Self {
        Self {
            logged_errors: Vec::new(),
        }
    }

    /// Logs an error with a string message.
    pub fn error(&mut self, msg: &str) {
        self.logged_errors.push(msg.to_string());
    }

    /// Logs an error with format string and arguments.
    pub fn errorf(&mut self, format: &str, args: &[&dyn std::fmt::Display]) {
        let msg = args.iter().fold(format.to_string(), |acc, arg| {
            acc.replacen("{}", &arg.to_string(), 1)
        });
        self.logged_errors.push(msg);
    }

    /// Returns the logged errors.
    pub fn get_logged_errors(&self) -> &[String] {
        &self.logged_errors
    }

    /// Asserts that the expected errors were logged.
    pub fn assert_errors(&self, expected: &[String]) {
        assert_eq!(
            expected, self.logged_errors,
            "Expected errors {:?} but got {:?}",
            expected, self.logged_errors
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fake_field_logger_error() {
        let mut logger = FakeFieldLogger::new();
        logger.error("something went wrong");
        assert_eq!(logger.get_logged_errors(), &["something went wrong"]);
    }

    #[test]
    fn test_fake_field_logger_errorf() {
        let mut logger = FakeFieldLogger::new();
        let value = "test_value";
        logger.errorf("error: {}", &[&value]);
        assert_eq!(logger.get_logged_errors(), &["error: test_value"]);
    }

    #[test]
    fn test_assert_errors_passes() {
        let logger = FakeFieldLogger::new();
        logger.assert_errors(&[]);
    }
}
