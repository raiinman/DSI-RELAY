use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterError {
    pub code: &'static str,
    pub message: String,
}

impl AdapterError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AdapterError {}
