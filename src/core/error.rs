use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyswhyError {
    InvalidQuery(String),
}

impl fmt::Display for SyswhyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidQuery(message) => write!(f, "invalid query: {message}"),
        }
    }
}

impl std::error::Error for SyswhyError {}
