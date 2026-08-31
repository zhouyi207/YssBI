//! Structured Bayesian model validation reports.

use serde::{Deserialize, Serialize, Serializer, ser::SerializeStruct};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationReport {
    errors: Vec<ValidationIssue>,
    warnings: Vec<ValidationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidationIssue {
    pub code: String,
    pub severity: ValidationSeverity,
    pub path: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValidationSeverity {
    Error,
    Warning,
}

impl ValidationReport {
    pub(crate) fn new(errors: Vec<ValidationIssue>, warnings: Vec<ValidationIssue>) -> Self {
        Self { errors, warnings }
    }

    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn errors(&self) -> &[ValidationIssue] {
        &self.errors
    }

    pub fn warnings(&self) -> &[ValidationIssue] {
        &self.warnings
    }

    pub(crate) fn with_error(mut self, code: &str, path: impl Into<String>) -> Self {
        self.errors.push(error(code, path));
        self
    }
}

impl Serialize for ValidationReport {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("ValidationReport", 3)?;
        state.serialize_field("ok", &self.is_ok())?;
        state.serialize_field("errors", &self.errors)?;
        state.serialize_field("warnings", &self.warnings)?;
        state.end()
    }
}

pub(crate) fn error(code: &str, path: impl Into<String>) -> ValidationIssue {
    ValidationIssue {
        code: code.to_string(),
        severity: ValidationSeverity::Error,
        path: path.into(),
    }
}

pub(crate) fn warning(code: &str, path: impl Into<String>) -> ValidationIssue {
    ValidationIssue {
        code: code.to_string(),
        severity: ValidationSeverity::Warning,
        path: path.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{ValidationIssue, ValidationReport, error};

    #[test]
    fn validation_report_derives_ok_from_its_error_set() {
        let report = ValidationReport::new(vec![error("dataset_required", "dataset")], Vec::new());

        assert!(!report.is_ok());
        assert_eq!(
            serde_json::to_value(report).expect("serialize validation report"),
            serde_json::json!({
                "ok": false,
                "errors": [{
                    "code": "dataset_required",
                    "severity": "error",
                    "path": "dataset"
                }],
                "warnings": []
            })
        );
    }

    #[test]
    fn validation_issue_wire_contains_only_stable_machine_fields() {
        let issue: ValidationIssue = serde_json::from_value(serde_json::json!({
            "code": "dataset_required",
            "severity": "error",
            "path": "dataset"
        }))
        .expect("deserialize safe validation issue");

        assert_eq!(
            serde_json::to_value(issue).expect("serialize safe validation issue"),
            serde_json::json!({
                "code": "dataset_required",
                "severity": "error",
                "path": "dataset"
            })
        );
        assert!(
            serde_json::from_value::<ValidationIssue>(serde_json::json!({
                "code": "dataset_required",
                "severity": "error",
                "path": "dataset",
                "message": "select a dataset",
                "hint": "open the dataset picker",
                "details": null
            }))
            .is_err(),
            "validation issue must reject legacy display fields"
        );
    }
}
