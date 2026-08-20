//! Scientific-computing error model.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SciError {
    InvalidInput(String),
}

impl SciError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput(_) => "sci_invalid_input",
        }
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput(message.into())
    }
}

impl fmt::Display for SciError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for SciError {}
