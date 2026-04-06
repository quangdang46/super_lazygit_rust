// Ported from ./references/lazygit-master/pkg/utils/errors.go

use std::fmt;

pub struct WrappedError {
    msg: String,
    source: Option<Box<dyn fmt::Error + Send + Sync>>,
}

impl WrappedError {
    pub fn new(err: &dyn fmt::Error, msg: &str) -> Self {
        Self {
            msg: msg.to_string(),
            source: Some(Box::new(err)),
        }
    }
}

impl fmt::Display for WrappedError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {:?}", self.msg, self.source)
    }
}

impl fmt::Debug for WrappedError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {:?}", self.msg, self.source)
    }
}

impl std::error::Error for WrappedError {}

pub fn wrap_error(
    err: Option<&dyn fmt::Error>,
) -> Option<Box<dyn std::error::Error + Send + Sync>> {
    err.map(|e| {
        let wrapped: Box<dyn std::error::Error + Send + Sync> =
            Box::new(WrappedError::new(e, "wrapped error"));
        wrapped
    })
}
