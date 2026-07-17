//! Scientific-computing error model.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SciError {
    InvalidInput(String),
    JuliaUnavailable(String),
    JuliaTaskFailed(String),
}

impl SciError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput(_) => "sci_invalid_input",
            Self::JuliaUnavailable(_) => "sci_julia_unavailable",
            Self::JuliaTaskFailed(_) => "sci_julia_task_failed",
        }
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput(message.into())
    }

    pub fn julia_unavailable(message: impl Into<String>) -> Self {
        Self::JuliaUnavailable(message.into())
    }

    pub fn julia_task_failed(message: impl Into<String>) -> Self {
        Self::JuliaTaskFailed(message.into())
    }
}

impl fmt::Display for SciError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message)
            | Self::JuliaUnavailable(message)
            | Self::JuliaTaskFailed(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for SciError {}
