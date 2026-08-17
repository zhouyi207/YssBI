use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub ok: bool,
    pub errors: Vec<ValidationIssue>,
    pub warnings: Vec<ValidationIssue>,
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
    pub fn new(errors: Vec<ValidationIssue>, warnings: Vec<ValidationIssue>) -> Self {
        Self {
            ok: errors.is_empty(),
            errors,
            warnings,
        }
    }
}

pub fn error(code: &str, path: impl Into<String>) -> ValidationIssue {
    ValidationIssue {
        code: code.to_string(),
        severity: ValidationSeverity::Error,
        path: path.into(),
    }
}

pub fn warning(code: &str, path: impl Into<String>) -> ValidationIssue {
    ValidationIssue {
        code: code.to_string(),
        severity: ValidationSeverity::Warning,
        path: path.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::ValidationIssue;

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
