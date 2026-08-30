//! Canonical persisted and transport contract for project computation settings.

use serde::{Deserialize, Serialize};
use yss_project_identity::{OperationId, ProjectInstanceId};

pub const DEFAULT_ABSOLUTE_TOLERANCE: f64 = 1e-12;
pub const DEFAULT_RELATIVE_TOLERANCE: f64 = 1e-9;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NumericTolerance {
    pub absolute: f64,
    pub relative: f64,
}

impl NumericTolerance {
    pub fn validate(&self) -> Result<(), ComputationSettingsValidationError> {
        if !self.absolute.is_finite()
            || !self.relative.is_finite()
            || self.absolute < 0.0
            || self.relative < 0.0
        {
            return Err(ComputationSettingsValidationError::InvalidTolerance);
        }
        if self.absolute == 0.0 && self.relative == 0.0 {
            return Err(ComputationSettingsValidationError::ZeroTolerance);
        }
        Ok(())
    }
}

impl Default for NumericTolerance {
    fn default() -> Self {
        Self {
            absolute: DEFAULT_ABSOLUTE_TOLERANCE,
            relative: DEFAULT_RELATIVE_TOLERANCE,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StatisticalMissingValuePolicy {
    #[default]
    Listwise,
    Reject,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NumericSettings {
    pub tolerance: NumericTolerance,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MissingValueSettings {
    pub statistics: StatisticalMissingValuePolicy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectComputationSettings {
    pub numeric: NumericSettings,
    pub missing_values: MissingValueSettings,
}

impl ProjectComputationSettings {
    pub fn validate(&self) -> Result<(), ComputationSettingsValidationError> {
        self.numeric.tolerance.validate()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputationSettingsValidationError {
    InvalidTolerance,
    ZeroTolerance,
}

impl std::fmt::Display for ComputationSettingsValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTolerance => {
                formatter.write_str("numeric tolerances must be finite and nonnegative")
            }
            Self::ZeroTolerance => {
                formatter.write_str("absolute and relative tolerances cannot both be zero")
            }
        }
    }
}

impl std::error::Error for ComputationSettingsValidationError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComputationSettingsSnapshot {
    pub project_instance_id: ProjectInstanceId,
    pub settings_revision: u64,
    pub publication_revision: u64,
    pub settings: ProjectComputationSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComputationSettingsMutationRequest {
    pub project_instance_id: ProjectInstanceId,
    pub operation_id: OperationId,
    pub expected_revision: u64,
    pub settings: ProjectComputationSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComputationSettingsMutationReceipt {
    pub project_instance_id: ProjectInstanceId,
    pub operation_id: OperationId,
    pub settings_revision: u64,
    pub publication_revision: u64,
    pub settings: ProjectComputationSettings,
}

#[cfg(test)]
mod tests {
    use super::{
        ComputationSettingsMutationRequest, ComputationSettingsValidationError, NumericTolerance,
        ProjectComputationSettings,
    };
    use serde_json::json;

    #[test]
    fn tolerance_validation_is_the_single_fail_closed_rule() {
        assert_eq!(
            NumericTolerance {
                absolute: f64::NAN,
                relative: 1.0,
            }
            .validate(),
            Err(ComputationSettingsValidationError::InvalidTolerance)
        );
        assert_eq!(
            NumericTolerance {
                absolute: -1.0,
                relative: 1.0,
            }
            .validate(),
            Err(ComputationSettingsValidationError::InvalidTolerance)
        );
        assert_eq!(
            NumericTolerance {
                absolute: 0.0,
                relative: 0.0,
            }
            .validate(),
            Err(ComputationSettingsValidationError::ZeroTolerance)
        );
        assert!(ProjectComputationSettings::default().validate().is_ok());
    }

    #[test]
    fn settings_and_mutation_wire_are_camel_case_and_strict() {
        assert_eq!(
            serde_json::to_value(ProjectComputationSettings::default()).unwrap(),
            json!({
                "numeric": {
                    "tolerance": {
                        "absolute": 1e-12,
                        "relative": 1e-9
                    }
                },
                "missingValues": { "statistics": "listwise" }
            })
        );

        let request = json!({
            "projectInstanceId": "project-a",
            "operationId": "00000000-0000-0000-0000-000000000701",
            "expectedRevision": 3,
            "settings": {
                "numeric": {
                    "tolerance": {
                        "absolute": 0.25,
                        "relative": 0.5
                    }
                },
                "missingValues": { "statistics": "reject" }
            }
        });
        let decoded: ComputationSettingsMutationRequest =
            serde_json::from_value(request.clone()).unwrap();
        assert_eq!(decoded.project_instance_id.as_str(), "project-a");
        assert_eq!(decoded.expected_revision, 3);

        let mut with_unknown = request;
        with_unknown["settings"]["numeric"]["legacyTolerance"] = json!(1.0);
        assert!(
            serde_json::from_value::<ComputationSettingsMutationRequest>(with_unknown).is_err(),
            "unknown nested settings fields must require an explicit schema change"
        );
    }
}
