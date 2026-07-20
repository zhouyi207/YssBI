use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub ok: bool,
    pub errors: Vec<ValidationIssue>,
    pub warnings: Vec<ValidationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    pub code: String,
    pub severity: ValidationSeverity,
    pub message: String,
    pub path: Option<String>,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValidationSeverity {
    Error,
    Warning,
}

impl ValidationReport {
    pub fn new(errors: Vec<ValidationIssue>, warnings: Vec<ValidationIssue>) -> Self {
        Self {
            ok: errors.is_empty(),
            errors,
            warnings,
        }
    }
}

pub fn error(code: &str, message: impl Into<String>, path: impl Into<String>) -> ValidationIssue {
    ValidationIssue {
        code: code.to_string(),
        severity: ValidationSeverity::Error,
        message: message.into(),
        path: Some(path.into()),
        hint: None,
    }
}

pub fn warning(code: &str, message: impl Into<String>, path: impl Into<String>) -> ValidationIssue {
    ValidationIssue {
        code: code.to_string(),
        severity: ValidationSeverity::Warning,
        message: message.into(),
        path: Some(path.into()),
        hint: None,
    }
}
